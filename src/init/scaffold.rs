use anyhow::{Context, Result};
use std::io::{self, BufRead, Write};
use std::path::Path;

/// Generates `.github/workflows/lint.yml` if it does not already exist.
/// Returns `true` if the file was written.
pub(super) fn generate_lint_workflow(
    project_root: &Path,
    base_branch: &str,
    has_rust: bool,
) -> Result<bool> {
    let workflows_dir = project_root.join(".github/workflows");
    let workflow_path = workflows_dir.join("lint.yml");
    if workflow_path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(&workflows_dir)?;
    let push_comment = if has_rust {
        " # warms the Rust cache so PR branches get a cache hit"
    } else {
        ""
    };
    let rust_steps = if has_rust {
        "\n      - uses: Swatinem/rust-cache@c19371144df3bb44fab255c43d04cbc2ab54d1c4 # v2.9.1\n\n      - name: Install Rust lint components\n        run: rustup component add clippy rustfmt\n"
    } else {
        ""
    };
    let content = format!(
        r#"name: Lint

on:
  push:
    branches: [{base_branch}]{push_comment}
  pull_request:
    branches: [{base_branch}]

permissions:
  contents: read

jobs:
  lint:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2
        with:
          persist-credentials: false
          fetch-depth: 0

      - name: Setup mise
        uses: jdx/mise-action@1648a7812b9aeae629881980618f079932869151 # v4.0.1
        with:
          version: v2026.4.19
          sha256: 6b58ff5f1e1ce98ed2b7e5372c344ea48182c460e5b6df12d9e0def35aad4438
{rust_steps}
      - name: Lint
        env:
          GITHUB_TOKEN: ${{{{ github.token }}}}
          GITHUB_HEAD_SHA: ${{{{ github.event.pull_request.head.sha }}}}
        run: mise run lint
"#
    );
    std::fs::write(&workflow_path, content)?;
    println!("  wrote {}", workflow_path.display());
    Ok(true)
}

/// Adds a `[tasks.<name>]` entry only when it is not already present.
/// Returns `true` if an entry was added.
fn add_task_if_absent(
    tasks: &mut toml_edit::Table,
    name: &str,
    description: &str,
    run: &str,
) -> bool {
    if tasks.contains_key(name) {
        return false;
    }
    write_task(tasks, name, description, run);
    true
}

/// Unconditionally writes a `[tasks.<name>]` entry (adds or replaces).
fn write_task(tasks: &mut toml_edit::Table, name: &str, description: &str, run: &str) {
    let mut t = toml_edit::Table::new();
    t.insert("description", toml_edit::value(description));
    t.insert("run", toml_edit::value(run));
    tasks.insert(name, toml_edit::Item::Table(t));
}

/// Returns `true` when the named task has a `depends` array where at least one
/// entry is in `removed_tasks`. Used to detect tasks made stale by v1 removal.
fn task_has_removed_dep(tasks: &toml_edit::Table, name: &str, removed: &[String]) -> bool {
    let Some(item) = tasks.get(name) else {
        return false;
    };
    let Some(task) = item.as_table() else {
        return false;
    };
    let Some(depends) = task.get("depends").and_then(|v| v.as_array()) else {
        return false;
    };
    depends.iter().any(|v| {
        v.as_str()
            .map(|s| removed.iter().any(|r| r == s))
            .unwrap_or(false)
    })
}

/// Adds `[env] FLINT_CONFIG_DIR` and the standard `lint*` tasks to `mise.toml`,
/// skipping any that are already present.
///
/// When `removed_v1_tasks` is non-empty, standard tasks whose `depends` reference
/// any of those removed tasks are replaced (they became stale after v1 removal).
///
/// Returns `true` if the file was changed.
pub(super) fn apply_env_and_tasks(
    mise_path: &Path,
    config_dir_rel: &str,
    _has_slow: bool,
    removed_v1_tasks: &[String],
) -> Result<bool> {
    let content = std::fs::read_to_string(mise_path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());
    let mut changed = false;

    // [env] — add FLINT_CONFIG_DIR if absent
    {
        if !doc.contains_key("env") {
            doc.insert("env", toml_edit::Item::Table(toml_edit::Table::new()));
        }
        let env = doc["env"].as_table_mut().context("[env] is not a table")?;
        if !env.contains_key("FLINT_CONFIG_DIR") {
            env.insert("FLINT_CONFIG_DIR", toml_edit::value(config_dir_rel));
            changed = true;
        }
    }

    // [tasks] — add lint / lint:fix
    {
        if !doc.contains_key("tasks") {
            let mut tasks_table = toml_edit::Table::new();
            tasks_table.set_implicit(true);
            doc.insert("tasks", toml_edit::Item::Table(tasks_table));
        }
        let tasks = doc["tasks"]
            .as_table_mut()
            .context("[tasks] is not a table")?;

        // Replace the lint task when it was made stale by v1 removal (its depends
        // referenced removed tasks and would now fail). Otherwise add if absent.
        let lint_stale = task_has_removed_dep(tasks, "lint", removed_v1_tasks);
        if lint_stale {
            write_task(tasks, "lint", "Run all lints", "flint run");
            changed = true;
        } else {
            changed |= add_task_if_absent(tasks, "lint", "Run all lints", "flint run");
        }

        changed |= add_task_if_absent(tasks, "lint:fix", "Auto-fix lint issues", "flint run --fix");
    }

    if changed {
        std::fs::write(mise_path, doc.to_string())?;
    }
    Ok(changed)
}

/// Offers to install the git pre-commit hook via `flint hook install`.
/// Prompts the user unless `yes` is true. Silently skips if the hook is already installed.
pub(super) fn maybe_install_hook(project_root: &Path, yes: bool) -> Result<()> {
    let hook_path = crate::hook::pre_commit_path(project_root)?;
    if hook_path.exists() {
        return Ok(());
    }

    let install = if yes {
        true
    } else {
        print!(
            "Install pre-commit hook (runs `flint run --fix --fast-only` before each commit)? [Y/n] "
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        !input.trim().eq_ignore_ascii_case("n")
    };

    if install {
        crate::hook::install(project_root)?;
    }
    Ok(())
}
