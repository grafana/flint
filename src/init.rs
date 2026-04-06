use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use crate::registry::{Category, Check, builtin};

/// Linter profile — controls which linters are included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Profile {
    /// Language-specific linters only (shellcheck, ruff, cargo-clippy, …).
    Lang,
    /// Lang + fast general linters (+ prettier, codespell, editorconfig-checker, …).
    Default,
    /// Default + slow linters (+ lychee, renovate-deps).
    Comprehensive,
}

/// Desired tools for a profile: maps each mise tool key to its optional components string.
type DesiredTools = HashMap<String, Option<&'static str>>;

pub fn run(project_root: &Path, profile_arg: Option<Profile>, yes: bool) -> Result<()> {
    println!(
        "Tip: flint init detects languages from tracked files (`git ls-files`). \
Add and stage your source files before running init so the detection is accurate."
    );
    println!();

    let registry = builtin();

    // Detect which file patterns have matches in the repo.
    let present_patterns = detect_present_patterns(project_root, &registry)?;

    // Choose profile interactively if not supplied via flag.
    let profile = match profile_arg {
        Some(p) => p,
        None => prompt_profile()?,
    };

    // Compute the map of mise tool keys → optional components this profile requires.
    let desired = compute_desired_tools(&registry, &present_patterns, profile);

    // Read existing mise.toml (may not exist yet).
    let mise_path = project_root.join("mise.toml");
    let current_content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    let current_tool_keys = parse_tool_keys(&current_content);

    // All flint-known tool keys — used to constrain what we remove.
    let known_keys: HashSet<&str> = registry.iter().filter_map(install_key).collect();

    let mut to_add: Vec<(String, Option<&'static str>)> = desired
        .iter()
        .filter(|(k, _)| !current_tool_keys.contains(k.as_str()))
        .map(|(k, c)| (k.clone(), *c))
        .collect();
    to_add.sort_by(|a, b| a.0.cmp(&b.0));

    let mut to_remove: Vec<String> = current_tool_keys
        .iter()
        .filter(|k| known_keys.contains(k.as_str()) && !desired.contains_key(k.as_str()))
        .cloned()
        .collect();
    to_remove.sort();

    // Tools already present that need components added (e.g. `rust = "1.x"` → inline table).
    let mut to_upgrade: Vec<(String, &'static str)> = desired
        .iter()
        .filter_map(|(k, components)| components.map(|c| (k.clone(), c)))
        .filter(|(k, _)| current_tool_keys.contains(k.as_str()))
        .filter(|(k, c)| entry_components_differ(&current_content, k, c))
        .collect();
    to_upgrade.sort_by(|a, b| a.0.cmp(&b.0));

    if to_add.is_empty() && to_remove.is_empty() && to_upgrade.is_empty() {
        println!("mise.toml [tools] is already up to date for the selected profile.");
        return Ok(());
    }

    // Show planned changes.
    println!();
    if !to_add.is_empty() {
        println!("Adding to [tools]:");
        for (key, components) in &to_add {
            println!("  + {} = {}", key, format_toml_value(components));
        }
    }
    if !to_upgrade.is_empty() {
        println!("Updating [tools] (adding components):");
        for (key, components) in &to_upgrade {
            println!("  ~ {key}: add components = \"{components}\"");
        }
    }
    if !to_remove.is_empty() {
        println!("Removing from [tools]:");
        for key in &to_remove {
            println!("  - {}", key);
        }
    }
    println!();

    if !yes && !confirm("Apply changes to mise.toml?")? {
        println!("Aborted.");
        return Ok(());
    }

    apply_changes(
        &mise_path,
        &current_content,
        &to_add,
        &to_remove,
        &to_upgrade,
    )?;
    println!("Done. Run `mise install` to install the new tools.");
    Ok(())
}

/// Returns the canonical mise.toml tool key to write when installing this check
/// via `flint init`, or `None` if no mise entry is needed (built-in or
/// unconditionally active checks).
///
/// Preference order: `mise_install_key` → `mise_tool_name` → `bin_name`.
pub fn install_key(check: &Check) -> Option<&'static str> {
    if !check.uses_binary() || check.activate_unconditionally {
        return None;
    }
    Some(
        check
            .mise_install_key
            .or(check.mise_tool_name)
            .unwrap_or(check.bin_name),
    )
}

/// Compute the map of `tool_key → optional_components` needed for `profile`
/// given the detected file patterns present in the repo.
fn compute_desired_tools(
    registry: &[Check],
    present_patterns: &HashSet<String>,
    profile: Profile,
) -> DesiredTools {
    let mut desired = DesiredTools::new();
    for check in registry {
        let key = match install_key(check) {
            Some(k) => k,
            None => continue,
        };
        if !files_present(check, present_patterns) {
            continue;
        }
        let included = match profile {
            Profile::Lang => check.category == Category::Lang,
            Profile::Default => check.category != Category::Slow,
            Profile::Comprehensive => true,
        };
        if included {
            desired.insert(key.to_string(), check.mise_install_components);
        }
    }
    desired
}

/// Returns `true` if the repo contains at least one file matching any of the
/// check's patterns. Checks with no patterns (project-scope specials like
/// lychee) are always considered present.
fn files_present(check: &Check, present_patterns: &HashSet<String>) -> bool {
    check.patterns.is_empty()
        || check
            .patterns
            .iter()
            .any(|p| *p == "*" || present_patterns.contains(*p))
}

/// Runs `git ls-files -- <pattern>` for every unique pattern in the registry
/// and returns the set of patterns that produced at least one result.
fn detect_present_patterns(project_root: &Path, registry: &[Check]) -> Result<HashSet<String>> {
    let all_patterns: HashSet<&str> = registry
        .iter()
        .flat_map(|c| c.patterns.iter().copied())
        .filter(|p| *p != "*")
        .collect();

    let mut present = HashSet::new();
    for pattern in all_patterns {
        let out = Command::new("git")
            .args(["ls-files", "--", pattern])
            .current_dir(project_root)
            .output()
            .context("git ls-files")?;
        if !out.stdout.is_empty() {
            present.insert(pattern.to_string());
        }
    }
    Ok(present)
}

/// Returns the set of keys currently declared in `[tools]`.
fn parse_tool_keys(content: &str) -> HashSet<String> {
    let value: toml::Value = match toml::from_str(content) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };
    value
        .get("tools")
        .and_then(|v| v.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default()
}

/// Returns `true` if the `[tools]` entry for `key` exists and its `components`
/// field is absent or differs from `required`. Used to detect entries that need
/// upgrading (missing components) or correcting (wrong components).
fn entry_components_differ(content: &str, key: &str, required: &str) -> bool {
    let doc: toml_edit::DocumentMut = match content.parse() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let tools = match doc.get("tools").and_then(|t| t.as_table()) {
        Some(t) => t,
        None => return false,
    };
    match tools.get(key) {
        Some(item) => match item.as_value() {
            Some(toml_edit::Value::InlineTable(tbl)) => {
                tbl.get("components").and_then(|v| v.as_str()) != Some(required)
            }
            Some(toml_edit::Value::String(_)) => true,
            _ => false,
        },
        None => false,
    }
}

/// Format the display string for a tool entry (used in the planned-changes output).
fn format_toml_value(components: &Option<&'static str>) -> String {
    match components {
        Some(c) => format!(r#"{{ version = "latest", components = "{c}" }}"#),
        None => r#""latest""#.to_string(),
    }
}

fn prompt_profile() -> Result<Profile> {
    println!("Select a profile:");
    println!("  1) lang          — language linters only (shellcheck, ruff, cargo-clippy, …)");
    println!(
        "  2) default       — lang + fast general linters (+ prettier, codespell, ec, lychee, …)"
    );
    println!("  3) comprehensive — default + slow linters (+ renovate-deps)");
    println!();
    print!("Profile [1-3, default: 2]: ");
    io::stdout().flush()?;

    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(match line.trim() {
        "1" => Profile::Lang,
        "3" => Profile::Comprehensive,
        _ => Profile::Default,
    })
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N]: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();
    Ok(answer == "y" || answer == "yes")
}

fn apply_changes(
    path: &Path,
    current_content: &str,
    to_add: &[(String, Option<&'static str>)],
    to_remove: &[String],
    to_upgrade: &[(String, &'static str)],
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
                tbl.insert("components", toml_edit::Value::from(*comps));
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

    // Upgrade existing entries: preserve the current version, add components.
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
        tbl.insert("components", toml_edit::Value::from(*components));
        tools.insert(
            key.as_str(),
            toml_edit::Item::Value(toml_edit::Value::InlineTable(tbl)),
        );
    }

    std::fs::write(path, doc.to_string())?;
    Ok(())
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_registry_checks_have_install_key_or_none() {
        // Every check that uses a binary and isn't unconditional must have a resolvable key.
        for check in builtin() {
            if check.uses_binary() && !check.activate_unconditionally {
                let key = install_key(&check);
                assert!(
                    key.is_some(),
                    "check '{}' is missing an install key",
                    check.name
                );
            }
        }
    }

    #[test]
    fn entry_components_differ_string_value() {
        let content = "[tools]\nrust = \"1.80.0\"\n";
        assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
    }

    #[test]
    fn entry_components_differ_inline_table_without_components() {
        let content = "[tools]\nrust = { version = \"1.80.0\" }\n";
        assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
    }

    #[test]
    fn entry_components_differ_inline_table_wrong_components() {
        let content = "[tools]\nrust = { version = \"1.80.0\", components = \"clippy\" }\n";
        assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
    }

    #[test]
    fn entry_components_differ_inline_table_correct_components() {
        let content = "[tools]\nrust = { version = \"1.80.0\", components = \"clippy,rustfmt\" }\n";
        assert!(!entry_components_differ(content, "rust", "clippy,rustfmt"));
    }

    #[test]
    fn apply_changes_upgrade_preserves_version() {
        let content = "[tools]\nrust = \"1.80.0\"\n";
        let tmp = tempfile::NamedTempFile::new().unwrap();
        apply_changes(
            tmp.path(),
            content,
            &[],
            &[],
            &[("rust".to_string(), "clippy,rustfmt")],
        )
        .unwrap();
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(result.contains("version = \"1.80.0\""), "version preserved");
        assert!(
            result.contains("components = \"clippy,rustfmt\""),
            "components added"
        );
    }

    #[test]
    fn parse_tool_keys_reads_simple_toml() {
        let content = r#"
[tools]
shellcheck = "v0.11.0"
"npm:prettier" = "3.8.1"
rust = { version = "1.0", components = "clippy" }
"#;
        let keys = parse_tool_keys(content);
        assert!(keys.contains("shellcheck"));
        assert!(keys.contains("npm:prettier"));
        assert!(keys.contains("rust"));
        assert!(!keys.contains("nonexistent"));
    }

    #[test]
    fn compute_desired_tools_lang_profile() {
        let registry = builtin();
        let mut present = HashSet::new();
        present.insert("*.sh".to_string());
        present.insert("*.bash".to_string());
        let tools = compute_desired_tools(&registry, &present, Profile::Lang);
        assert!(tools.contains_key("shellcheck"));
        assert!(tools.contains_key("shfmt"));
        // codespell is not lang-only
        assert!(!tools.contains_key("pipx:codespell"));
    }

    #[test]
    fn rust_install_entry_has_components() {
        let registry = builtin();
        let mut present = HashSet::new();
        present.insert("*.rs".to_string());
        let tools = compute_desired_tools(&registry, &present, Profile::Lang);
        // Both cargo-clippy and cargo-fmt share the "rust" key with components set.
        assert_eq!(
            tools.get("rust"),
            Some(&Some("clippy,rustfmt")),
            "rust tool entry should carry components"
        );
    }

    #[test]
    fn compute_desired_tools_default_excludes_slow() {
        let registry = builtin();
        let present: HashSet<String> = HashSet::new();
        let tools = compute_desired_tools(&registry, &present, Profile::Default);
        // renovate-deps is slow — should be absent
        assert!(!tools.contains_key("npm:renovate"));
        // lychee is fast — should be present (empty patterns → always present)
        assert!(tools.contains_key("lychee"));
    }

    #[test]
    fn compute_desired_tools_comprehensive_includes_slow() {
        let registry = builtin();
        let present: HashSet<String> = HashSet::new();
        let tools = compute_desired_tools(&registry, &present, Profile::Comprehensive);
        assert!(tools.contains_key("lychee"));
        assert!(tools.contains_key("npm:renovate"));
    }
}
