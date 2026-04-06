use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use crate::registry::{Category, Check, builtin};

/// Linter profile — shorthand for `--profile` CLI flag; maps to a category set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Profile {
    /// Primary language linters only (ruff, cargo-clippy, golangci-lint, …).
    Lang,
    /// Lang + supplementary checks + fast general tools (shellcheck, prettier, codespell, …).
    Default,
    /// Default + slow linters (renovate-deps).
    Comprehensive,
}

fn profile_to_categories(profile: Profile) -> HashSet<Category> {
    match profile {
        Profile::Lang => [Category::Lang].into(),
        Profile::Default => [Category::Lang, Category::Style, Category::Default].into(),
        Profile::Comprehensive => [
            Category::Lang,
            Category::Style,
            Category::Default,
            Category::Slow,
        ]
        .into(),
    }
}

/// Desired tools for a profile: maps each mise tool key to its optional components string.
type DesiredTools = HashMap<String, Option<&'static str>>;

// --- Change list (step 2) ---

enum ChangeKind {
    Add {
        key: String,
        components: Option<&'static str>,
    },
    Remove {
        key: String,
    },
    Upgrade {
        key: String,
        components: &'static str,
    },
}

struct ChangeItem {
    selected: bool,
    kind: ChangeKind,
}

impl ChangeItem {
    fn label(&self) -> String {
        match &self.kind {
            ChangeKind::Add { key, components } => {
                format!("[+]  {} = {}", key, format_toml_value(components))
            }
            ChangeKind::Remove { key } => format!("[-]  {}", key),
            ChangeKind::Upgrade { key, components } => {
                format!("[~]  {} (add components: {})", key, components)
            }
        }
    }
}

// --- Category selection (step 1) ---

struct CategoryItem {
    selected: bool,
    category: Category,
    label: &'static str,
}

fn default_category_items() -> Vec<CategoryItem> {
    vec![
        CategoryItem {
            selected: true,
            category: Category::Lang,
            label: "lang    — primary language linters (ruff, cargo-clippy, golangci-lint, …)",
        },
        CategoryItem {
            selected: true,
            category: Category::Style,
            label: "style   — supplementary checks (shellcheck, actionlint, hadolint, …)",
        },
        CategoryItem {
            selected: true,
            category: Category::Default,
            label: "general — general tools (codespell, ec, lychee, …)",
        },
        CategoryItem {
            selected: false,
            category: Category::Slow,
            label: "slow    — slow linters (renovate-deps)",
        },
    ]
}

pub fn run(project_root: &Path, profile_arg: Option<Profile>, yes: bool) -> Result<()> {
    println!(
        "Tip: flint init detects languages from tracked files (`git ls-files`). \
Add and stage your source files before running init so the detection is accurate."
    );
    println!();

    let registry = builtin();

    // Detect which file patterns have matches in the repo.
    let present_patterns = detect_present_patterns(project_root, &registry)?;

    // Determine the active category set.
    // --profile maps directly; otherwise ask interactively (skipped with --yes, uses default).
    let categories: HashSet<Category> = if let Some(profile) = profile_arg {
        profile_to_categories(profile)
    } else if yes {
        // --yes without --profile: use the default category set (lang + style + general).
        profile_to_categories(Profile::Default)
    } else {
        let mut cat_items = default_category_items();
        if !select_categories(&mut cat_items)? {
            println!("Aborted.");
            return Ok(());
        }
        println!();
        cat_items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.category)
            .collect()
    };

    // Compute the map of mise tool keys → optional components for the selected categories.
    let desired = compute_desired_tools(&registry, &present_patterns, &categories);

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

    // Build unified change list.
    let mut items: Vec<ChangeItem> = Vec::new();
    for (key, components) in &to_add {
        items.push(ChangeItem {
            selected: true,
            kind: ChangeKind::Add {
                key: key.clone(),
                components: *components,
            },
        });
    }
    for key in &to_remove {
        items.push(ChangeItem {
            selected: true,
            kind: ChangeKind::Remove { key: key.clone() },
        });
    }
    for (key, components) in &to_upgrade {
        items.push(ChangeItem {
            selected: true,
            kind: ChangeKind::Upgrade {
                key: key.clone(),
                components,
            },
        });
    }

    if items.is_empty() {
        println!("mise.toml [tools] is already up to date for the selected categories.");
        return Ok(());
    }

    // Interactive item selection (skipped with --yes).
    if !yes && !interactive_select(&mut items)? {
        println!("Aborted.");
        return Ok(());
    }

    let final_add: Vec<(String, Option<&'static str>)> = items
        .iter()
        .filter(|i| i.selected)
        .filter_map(|i| match &i.kind {
            ChangeKind::Add { key, components } => Some((key.clone(), *components)),
            _ => None,
        })
        .collect();
    let final_remove: Vec<String> = items
        .iter()
        .filter(|i| i.selected)
        .filter_map(|i| match &i.kind {
            ChangeKind::Remove { key } => Some(key.clone()),
            _ => None,
        })
        .collect();
    let final_upgrade: Vec<(String, &'static str)> = items
        .iter()
        .filter(|i| i.selected)
        .filter_map(|i| match &i.kind {
            ChangeKind::Upgrade { key, components } => Some((key.clone(), *components)),
            _ => None,
        })
        .collect();

    if final_add.is_empty() && final_remove.is_empty() && final_upgrade.is_empty() {
        println!("No changes selected.");
        return Ok(());
    }

    apply_changes(
        &mise_path,
        &current_content,
        &final_add,
        &final_remove,
        &final_upgrade,
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

/// Compute the map of `tool_key → optional_components` for the given category set,
/// filtered to file patterns present in the repo.
fn compute_desired_tools(
    registry: &[Check],
    present_patterns: &HashSet<String>,
    categories: &HashSet<Category>,
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
        if categories.contains(&check.category) {
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

/// Step 1: interactive category selection. Returns `true` to continue, `false` to abort.
fn select_categories(items: &mut [CategoryItem]) -> Result<bool> {
    loop {
        println!("Select categories:");
        println!();
        for (i, item) in items.iter().enumerate() {
            let check = if item.selected { "✓" } else { " " };
            println!("  {:>2}. {}  {}", i + 1, check, item.label);
        }
        print!("\nToggle by number (space-separated), Enter to continue, q to abort: ");
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case("q") {
            return Ok(false);
        }
        if trimmed.is_empty() {
            return Ok(true);
        }
        for token in trimmed.split_whitespace() {
            if let Ok(n) = token.parse::<usize>()
                && n >= 1
                && n <= items.len()
            {
                items[n - 1].selected = !items[n - 1].selected;
            }
        }
        println!();
    }
}

/// Step 2: interactive change-list selection. Returns `true` to apply, `false` to abort.
fn interactive_select(items: &mut [ChangeItem]) -> Result<bool> {
    loop {
        print_items(items);
        print!("\nToggle by number (space-separated), Enter to apply, q to abort: ");
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case("q") {
            return Ok(false);
        }
        if trimmed.is_empty() {
            return Ok(true);
        }
        for token in trimmed.split_whitespace() {
            if let Ok(n) = token.parse::<usize>()
                && n >= 1
                && n <= items.len()
            {
                items[n - 1].selected = !items[n - 1].selected;
            }
        }
    }
}

fn print_items(items: &[ChangeItem]) {
    println!("\nRecommended changes:");
    println!();
    for (i, item) in items.iter().enumerate() {
        let check = if item.selected { "✓" } else { " " };
        println!("  {:>2}. {}  {}", i + 1, check, item.label());
    }
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
        present.insert("*.rs".to_string());
        let categories = profile_to_categories(Profile::Lang);
        let tools = compute_desired_tools(&registry, &present, &categories);
        // Shell checks are supplementary (Style), not included in the lang profile.
        assert!(!tools.contains_key("shellcheck"));
        assert!(!tools.contains_key("shfmt"));
        // Primary language linters are included.
        assert!(tools.contains_key("rust"));
        // General tools are not lang-only.
        assert!(!tools.contains_key("pipx:codespell"));
    }

    #[test]
    fn rust_install_entry_has_components() {
        let registry = builtin();
        let mut present = HashSet::new();
        present.insert("*.rs".to_string());
        let categories = profile_to_categories(Profile::Lang);
        let tools = compute_desired_tools(&registry, &present, &categories);
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
        let categories = profile_to_categories(Profile::Default);
        let tools = compute_desired_tools(&registry, &present, &categories);
        // renovate-deps is slow — should be absent
        assert!(!tools.contains_key("npm:renovate"));
        // lychee is fast — should be present (empty patterns → always present)
        assert!(tools.contains_key("lychee"));
    }

    #[test]
    fn compute_desired_tools_comprehensive_includes_slow() {
        let registry = builtin();
        // Must include renovate config pattern so renovate-deps is considered present.
        let mut present: HashSet<String> = HashSet::new();
        present.insert(".github/renovate.json5".to_string());
        let categories = profile_to_categories(Profile::Comprehensive);
        let tools = compute_desired_tools(&registry, &present, &categories);
        assert!(tools.contains_key("lychee"));
        assert!(tools.contains_key("npm:renovate"));
    }

    #[test]
    fn renovate_deps_absent_without_renovate_config() {
        let registry = builtin();
        // No renovate config file in present patterns → renovate-deps should be excluded.
        let present: HashSet<String> = HashSet::new();
        let categories = profile_to_categories(Profile::Comprehensive);
        let tools = compute_desired_tools(&registry, &present, &categories);
        assert!(!tools.contains_key("npm:renovate"));
    }
}
