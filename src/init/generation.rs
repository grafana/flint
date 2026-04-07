use anyhow::{Context, Result};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use super::LinterGroup;

pub(super) fn apply_changes(
    path: &Path,
    current_content: &str,
    to_add: &[(String, Option<String>)],
    to_remove: &[String],
    to_upgrade: &[(String, String)],
) -> Result<()> {
    let mut doc: toml_edit::DocumentMut = current_content
        .parse()
        .unwrap_or_else(|_| toml_edit::DocumentMut::new());

    // Ensure [tools] table exists.
    if !doc.contains_key("tools") {
        doc.insert("tools", toml_edit::Item::Table(toml_edit::Table::new()));
    }
    let tools = doc["tools"]
        .as_table_mut()
        .context("[tools] is not a table")?;

    for key in to_remove {
        tools.remove(key.as_str());
    }

    for (key, components) in to_add {
        match components {
            Some(comps) => {
                let mut tbl = toml_edit::InlineTable::new();
                tbl.insert("version", toml_edit::Value::from("latest"));
                tbl.insert("components", toml_edit::Value::from(comps.as_str()));
                tools.insert(
                    key.as_str(),
                    toml_edit::Item::Value(toml_edit::Value::InlineTable(tbl)),
                );
            }
            None => {
                tools.insert(key.as_str(), toml_edit::value("latest"));
            }
        }
    }

    // Upgrade existing entries: preserve the current version, update components.
    for (key, components) in to_upgrade {
        let existing_version = tools
            .get(key.as_str())
            .and_then(|item| item.as_value())
            .and_then(|v| match v {
                toml_edit::Value::String(s) => Some(s.value().to_string()),
                toml_edit::Value::InlineTable(tbl) => tbl
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                _ => None,
            })
            .unwrap_or_else(|| "latest".to_string());

        let mut tbl = toml_edit::InlineTable::new();
        tbl.insert("version", toml_edit::Value::from(existing_version.as_str()));
        tbl.insert("components", toml_edit::Value::from(components.as_str()));
        tools.insert(
            key.as_str(),
            toml_edit::Item::Value(toml_edit::Value::InlineTable(tbl)),
        );
    }

    std::fs::write(path, doc.to_string())?;
    Ok(())
}

/// Returns true if any currently-selected check has `Category::Slow`.
pub(super) fn has_slow_selected(groups: &[LinterGroup]) -> bool {
    use crate::registry::Category;
    groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.category == Category::Slow)
    })
}

/// Reads the default branch for `origin` from git, falling back to `"main"`.
pub(super) fn detect_base_branch(project_root: &Path) -> String {
    Command::new("git")
        .args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().strip_prefix("origin/").map(str::to_string))
        .unwrap_or_else(|| "main".to_string())
}

/// Reads `FLINT_CONFIG_DIR` from the `[env]` section of a mise.toml string, if present.
pub(super) fn get_existing_config_dir(content: &str) -> Option<String> {
    let doc: toml_edit::DocumentMut = content.parse().ok()?;
    doc.get("env")?
        .as_table()?
        .get("FLINT_CONFIG_DIR")?
        .as_str()
        .map(str::to_string)
}

/// Asks where `flint.toml` should live. Skips the prompt when `--yes` or when
/// `FLINT_CONFIG_DIR` is already set in the current mise.toml.
///
/// Returns a path relative to the project root (e.g. `".github/config"`).
pub(super) fn prompt_config_dir(existing: Option<&str>, yes: bool) -> Result<String> {
    if let Some(dir) = existing {
        return Ok(dir.to_string());
    }
    if yes {
        return Ok(".github/config".to_string());
    }

    const CHOICES: &[&str] = &[".github/config", ".github", ".", "other…"];
    println!("Where should flint.toml live?\n");
    for (i, choice) in CHOICES.iter().enumerate() {
        println!("  {}) {}", i + 1, choice);
    }
    print!("\nChoice [1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let input = input.trim();

    let idx: usize = if input.is_empty() {
        0
    } else {
        input.parse::<usize>().unwrap_or(1).saturating_sub(1)
    };

    if idx == CHOICES.len() - 1 {
        print!("Config dir path: ");
        io::stdout().flush()?;
        let mut path = String::new();
        io::stdin().lock().read_line(&mut path)?;
        Ok(path.trim().to_string())
    } else {
        Ok(CHOICES[idx.min(CHOICES.len() - 2)].to_string())
    }
}

/// Writes a skeleton `flint.toml` in `config_dir`. Creates the directory if needed.
/// Returns `true` if the file was written, `false` if it already existed.
pub(super) fn generate_flint_toml(
    config_dir: &Path,
    base_branch: &str,
    has_renovate: bool,
) -> Result<bool> {
    let toml_path = config_dir.join("flint.toml");
    if toml_path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(config_dir)?;
    let mut content = String::from("[settings]\n");
    if base_branch != "main" {
        content.push_str(&format!("base_branch = \"{base_branch}\"\n"));
    }
    content.push_str("# exclude = \"CHANGELOG\\\\.md\"\n");
    content.push_str("# exclude_paths = []\n");
    if has_renovate {
        content.push_str("\n[checks.renovate-deps]\n");
        content.push_str("# exclude_managers = []\n");
    }
    std::fs::write(&toml_path, &content)?;
    println!("  wrote {}", toml_path.display());
    Ok(true)
}

/// Generates `.github/workflows/lint.yml` if it does not already exist.
/// Returns `true` if the file was written.
pub(super) fn generate_lint_workflow(project_root: &Path, base_branch: &str) -> Result<bool> {
    let workflows_dir = project_root.join(".github/workflows");
    let workflow_path = workflows_dir.join("lint.yml");
    if workflow_path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(&workflows_dir)?;
    let content = format!(
        r#"name: Lint

on:
  push:
    branches: [{base_branch}]
  pull_request:
    branches: [{base_branch}]

permissions:
  contents: read

jobs:
  lint:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6
        with:
          persist-credentials: false
          fetch-depth: 0

      - name: Setup mise
        uses: jdx/mise-action@1648a7812b9aeae629881980618f079932869151 # v4.0.1
        with:
          version: v2026.4.1
          sha256: c597fa1e4da76d1ea1967111d150a6a655ca51a72f4cd17fdc584be2b9eaa8bd

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
    let mut t = toml_edit::Table::new();
    t.insert("description", toml_edit::value(description));
    t.insert("run", toml_edit::value(run));
    tasks.insert(name, toml_edit::Item::Table(t));
    true
}

/// Adds `[env] FLINT_CONFIG_DIR` and the standard `lint*` / `setup:pre-commit-hook`
/// tasks to `mise.toml`, skipping any that are already present.
///
/// Returns `true` if the file was changed.
pub(super) fn apply_env_and_tasks(
    mise_path: &Path,
    config_dir_rel: &str,
    has_slow: bool,
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

    // [tasks] — add lint / lint:fix / (lint:pre-commit) / setup:pre-commit-hook
    {
        if !doc.contains_key("tasks") {
            doc.insert("tasks", toml_edit::Item::Table(toml_edit::Table::new()));
        }
        let tasks = doc["tasks"]
            .as_table_mut()
            .context("[tasks] is not a table")?;

        changed |= add_task_if_absent(tasks, "lint", "Run all lints", "flint run");
        changed |= add_task_if_absent(tasks, "lint:fix", "Auto-fix lint issues", "flint run --fix");
        if has_slow {
            changed |= add_task_if_absent(
                tasks,
                "lint:pre-commit",
                "Fast auto-fix lint (skips slow checks) — for pre-commit/pre-push hooks",
                "flint run --fix --fast-only",
            );
        }
        let hook_task = if has_slow { "lint:pre-commit" } else { "lint" };
        changed |= add_task_if_absent(
            tasks,
            "setup:pre-commit-hook",
            "Install git pre-commit hook",
            &format!("mise generate git-pre-commit --write --task={hook_task}"),
        );
    }

    if changed {
        std::fs::write(mise_path, doc.to_string())?;
    }
    Ok(changed)
}

/// Installs the git pre-commit hook by running `mise generate git-pre-commit`.
/// Prompts the user unless `yes` is true. Silently skips if the hook is already installed.
pub(super) fn maybe_install_hook(project_root: &Path, hook_task: &str, yes: bool) -> Result<()> {
    let hook_path = project_root.join(".git/hooks/pre-commit");
    if hook_path.exists() {
        return Ok(());
    }

    let install = if yes {
        true
    } else {
        print!("Install pre-commit hook (runs `mise run {hook_task}` before each commit)? [Y/n] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        !input.trim().eq_ignore_ascii_case("n")
    };

    if install {
        let status = Command::new("mise")
            .args([
                "generate",
                "git-pre-commit",
                "--write",
                &format!("--task={hook_task}"),
            ])
            .current_dir(project_root)
            .status();
        match status {
            Ok(s) if s.success() => println!("  installed pre-commit hook"),
            _ => println!(
                "  warning: could not install pre-commit hook — run `mise run setup:pre-commit-hook` later"
            ),
        }
    }
    Ok(())
}
