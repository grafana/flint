use anyhow::Result;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use super::LinterGroup;

pub(super) use super::mise_tools::{
    apply_changes, ensure_flint_self_pin, ensure_node_for_npm, remove_tool_keys,
};
pub(crate) use super::mise_tools::{
    normalize_tools_section, replace_obsolete_keys, tools_section_needs_normalization,
};
pub(super) use super::v1::remove_v1_tasks;

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
