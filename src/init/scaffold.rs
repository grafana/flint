use anyhow::{Context, Result};
use std::io::{self, BufRead, Write};
use std::path::Path;

const AGENT_LINTING_SENTINEL: &str = "Run `mise run lint:fix` before committing changes.";
const AGENT_LINTING_BLOCK: &str = "## Linting\n\nRun `mise run lint:fix` before committing changes.\nIf output includes `fixed`, keep those changes.\nIf output includes `partial` or `review`, address the remaining issues and\nrun `mise run lint:fix` again.\n\nExample output:\nflint: fixed: gofmt — commit before pushing | partial: cargo-clippy\n";

/// Generates `.github/workflows/lint.yml` if it does not already exist.
/// Returns `true` if the file was written.
pub(super) fn generate_lint_workflow(
    project_root: &Path,
    base_branch: &str,
    needs_rust_components: bool,
) -> Result<bool> {
    let workflows_dir = project_root.join(".github/workflows");
    let workflow_path = workflows_dir.join("lint.yml");
    if workflow_path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(&workflows_dir)?;
    let push_comment = if needs_rust_components {
        " # warms the Rust cache so PR branches get a cache hit"
    } else {
        ""
    };
    let rust_steps = if needs_rust_components {
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

permissions: {{}}

jobs:
  lint:
    runs-on: ubuntu-24.04

    permissions:
      contents: read

    steps:
      - uses: actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10 # v6.0.3
        with:
          persist-credentials: false
          fetch-depth: 0

      - name: Setup mise
        uses: jdx/mise-action@e6a8b3978addb5a52f2b4cd9d91eafa7f0ab959d # v4.2.0
        with:
          version: v2026.6.5
          sha256: 9ca3e4e25c26c64886d036fe9ddb2e5415a204f2d5b9c35bf67abd4f15f0f768
{rust_steps}
      - name: Lint
        env:
          GITHUB_REPOSITORY: ${{{{ github.repository }}}}
          GITHUB_BASE_REF: ${{{{ github.base_ref }}}}
          GITHUB_HEAD_REF: ${{{{ github.head_ref }}}}
          PR_HEAD_REPO: ${{{{ github.event.pull_request.head.repo.full_name || github.repository }}}}
          GITHUB_TOKEN: ${{{{ github.token }}}}
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

/// Adds `[env] FLINT_CONFIG_DIR` and the standard `lint*` tasks to `mise.toml`,
/// skipping any that are already present.
///
/// Returns `true` if the file was changed.
pub(super) fn apply_env_and_tasks(
    mise_path: &Path,
    config_dir_rel: &str,
    _has_slow: bool,
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

        changed |= add_task_if_absent(tasks, "lint", "Run all lints", "flint run");
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
        print!("Install pre-commit hook (runs `flint run --fix` before each commit)? [Y/n] ");
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

/// Ensures agent guidance exists in AGENTS.md or CLAUDE.md.
///
/// Preference order:
/// - patch existing `AGENTS.md`
/// - else patch existing `CLAUDE.md`
/// - else create `AGENTS.md`
///
/// Returns `true` if a file was written.
pub(super) fn ensure_agent_linting_guidance(project_root: &Path) -> Result<bool> {
    let agents_path = project_root.join("AGENTS.md");
    let claude_path = project_root.join("CLAUDE.md");
    let target = if agents_path.exists() {
        agents_path
    } else if claude_path.exists() {
        claude_path
    } else {
        agents_path
    };

    let existing = std::fs::read_to_string(&target).unwrap_or_default();
    if existing.contains(AGENT_LINTING_SENTINEL) {
        return Ok(false);
    }

    let had_existing_content = !existing.is_empty();
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    if !content.is_empty() {
        content.push('\n');
    }
    content.push_str(AGENT_LINTING_BLOCK);
    std::fs::write(&target, content)?;
    let action = if had_existing_content {
        "patched"
    } else {
        "wrote"
    };
    println!("  {action} {}", target.display());
    Ok(true)
}
