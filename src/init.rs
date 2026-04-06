use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, ClearType},
};

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
#[cfg(test)]
type DesiredTools = HashMap<String, Option<&'static str>>;

// One entry per install key — groups all checks sharing that key.
struct LinterGroup<'a> {
    key: &'static str,
    checks: Vec<&'a Check>, // sorted by name
    installed: bool,
    needs_upgrade: bool,
    selected: bool,
}

impl LinterGroup<'_> {
    fn action(&self) -> &'static str {
        if self.selected {
            if !self.installed {
                "add"
            } else if self.needs_upgrade {
                "upgrade"
            } else {
                "keep"
            }
        } else if self.installed {
            "remove"
        } else {
            ""
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
    let present_patterns = detect_present_patterns(project_root, &registry)?;

    // Step 1: determine which categories set the initial pre-selection.
    let default_categories: HashSet<Category> = if let Some(profile) = profile_arg {
        profile_to_categories(profile)
    } else if yes {
        profile_to_categories(Profile::Default)
    } else {
        let mut cat_items = default_category_items();
        if !select_categories_arrow(&mut cat_items)? {
            println!("Aborted.");
            return Ok(());
        }
        cat_items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.category)
            .collect()
    };

    let mise_path = project_root.join("mise.toml");
    let current_content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    let current_tool_keys = parse_tool_keys(&current_content);
    let known_keys: HashSet<&str> = registry.iter().filter_map(install_key).collect();

    // Step 2: build one group per install key, covering all checks whose files are
    // present in the repo or which are already installed.
    let mut groups = build_linter_groups(
        &registry,
        &present_patterns,
        &current_tool_keys,
        &current_content,
        &default_categories,
    );

    if groups.is_empty() {
        println!("No applicable linters found for this project.");
        return Ok(());
    }

    // Step 3: interactive linter table (skipped with --yes).
    if !yes && !interactive_select_linters(&mut groups)? {
        println!("Aborted.");
        return Ok(());
    }

    // Derive changes from final selection state.
    let mut final_add: Vec<(String, Option<&'static str>)> = Vec::new();
    let mut final_remove: Vec<String> = Vec::new();
    let mut final_upgrade: Vec<(String, &'static str)> = Vec::new();

    for group in &groups {
        if group.selected {
            if !group.installed {
                let components = group.checks.iter().find_map(|c| c.mise_install_components);
                final_add.push((group.key.to_string(), components));
            } else if group.needs_upgrade {
                let components = group
                    .checks
                    .iter()
                    .find_map(|c| c.mise_install_components)
                    .unwrap();
                final_upgrade.push((group.key.to_string(), components));
            }
        } else if group.installed && known_keys.contains(group.key) {
            final_remove.push(group.key.to_string());
        }
    }

    if final_add.is_empty() && final_remove.is_empty() && final_upgrade.is_empty() {
        println!("No changes to apply.");
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
#[cfg(test)]
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

/// Builds one `LinterGroup` per install key, covering all checks whose file patterns
/// are present in the repo or whose key is already installed.
fn build_linter_groups<'a>(
    registry: &'a [Check],
    present_patterns: &HashSet<String>,
    current_tool_keys: &HashSet<String>,
    current_content: &str,
    default_categories: &HashSet<Category>,
) -> Vec<LinterGroup<'a>> {
    let mut by_key: HashMap<&'static str, Vec<&'a Check>> = HashMap::new();
    for check in registry {
        let key = match install_key(check) {
            Some(k) => k,
            None => continue,
        };
        if files_present(check, present_patterns) || current_tool_keys.contains(key) {
            by_key.entry(key).or_default().push(check);
        }
    }

    let mut groups: Vec<LinterGroup<'a>> = by_key
        .into_iter()
        .map(|(key, mut checks)| {
            checks.sort_by_key(|c| c.name);
            let installed = current_tool_keys.contains(key);
            let needs_upgrade = checks.iter().any(|c| {
                c.mise_install_components
                    .is_some_and(|comp| entry_components_differ(current_content, key, comp))
            });
            // Pre-select if any check in the group is in the default categories and its
            // patterns are present, OR if the key is already installed.
            let suggested = checks.iter().any(|c| {
                default_categories.contains(&c.category) && files_present(c, present_patterns)
            });
            LinterGroup {
                key,
                checks,
                installed,
                needs_upgrade,
                selected: suggested || installed,
            }
        })
        .collect();

    groups.sort_by_key(|g| g.checks.first().map_or(g.key, |c| c.name));
    groups
}

fn run_arrow_selector<T>(
    items: &mut Vec<T>,
    print_fn: fn(&mut dyn Write, &[T], usize) -> Result<usize>,
    toggle_fn: fn(&mut T),
) -> Result<bool> {
    let mut cursor = 0usize;
    terminal::enable_raw_mode()?;
    let result = (|| -> Result<bool> {
        let mut stdout = io::stdout();
        let mut n_lines = print_fn(&mut stdout, items, cursor)?;
        loop {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up if cursor > 0 => cursor -= 1,
                    KeyCode::Down if cursor + 1 < items.len() => cursor += 1,
                    KeyCode::Char(' ') => toggle_fn(&mut items[cursor]),
                    KeyCode::Enter => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(true);
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(false);
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        execute!(
                            stdout,
                            cursor::MoveUp(n_lines as u16),
                            terminal::Clear(ClearType::FromCursorDown)
                        )?;
                        return Ok(false);
                    }
                    _ => continue,
                }
                execute!(
                    stdout,
                    cursor::MoveUp(n_lines as u16),
                    terminal::Clear(ClearType::FromCursorDown)
                )?;
                n_lines = print_fn(&mut stdout, items, cursor)?;
            }
        }
    })();
    let _ = terminal::disable_raw_mode();
    println!();
    result
}

// --- Step 1: category selection ---

fn select_categories_arrow(items: &mut Vec<CategoryItem>) -> Result<bool> {
    run_arrow_selector(items, print_cat_selector, |item| {
        item.selected = !item.selected
    })
}

fn print_cat_selector(
    stdout: &mut dyn Write,
    items: &[CategoryItem],
    cursor: usize,
) -> Result<usize> {
    let mut lines = 0usize;
    write!(stdout, "Select categories:\r\n\r\n")?;
    lines += 2;
    for (i, item) in items.iter().enumerate() {
        let arrow = if i == cursor { ">" } else { " " };
        let sel = if item.selected { "✓" } else { " " };
        write!(stdout, "  {}  [{}]  {}\r\n", arrow, sel, item.label)?;
        lines += 1;
    }
    write!(
        stdout,
        "\r\n  ↑↓ navigate   space toggle   enter continue   q abort\r\n"
    )?;
    lines += 2;
    stdout.flush()?;
    Ok(lines)
}

// --- Step 2: linter table selection ---

fn interactive_select_linters(groups: &mut Vec<LinterGroup>) -> Result<bool> {
    run_arrow_selector(groups, print_linter_table, |group| {
        group.selected = !group.selected
    })
}

fn print_linter_table(
    stdout: &mut dyn Write,
    groups: &[LinterGroup],
    cursor: usize,
) -> Result<usize> {
    let name_w = groups
        .iter()
        .flat_map(|g| &g.checks)
        .map(|c| c.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let bin_w = groups
        .iter()
        .flat_map(|g| &g.checks)
        .map(|c| c.bin_name.len())
        .max()
        .unwrap_or(6)
        .max(6);

    let mut lines = 0usize;
    write!(
        stdout,
        "     {:<5}  {:<name_w$}  {:<bin_w$}  {:<4}  {:<30}  ACTION\r\n",
        "SEL",
        "NAME",
        "BINARY",
        "SPEED",
        "PATTERNS",
        name_w = name_w,
        bin_w = bin_w,
    )?;
    write!(
        stdout,
        "     {}\r\n",
        "-".repeat(5 + 2 + name_w + 2 + bin_w + 2 + 4 + 2 + 30 + 2 + 6)
    )?;
    lines += 2;

    for (gi, group) in groups.iter().enumerate() {
        let action = group.action();
        let sel_mark = if group.selected { "[✓]" } else { "[ ]" };
        for (ci, check) in group.checks.iter().enumerate() {
            let cursor_mark = if gi == cursor && ci == 0 { ">" } else { " " };
            let speed = if check.category == Category::Slow {
                "slow"
            } else {
                "fast"
            };
            let patterns = check.patterns.join(" ");
            write!(
                stdout,
                "  {}  {}  {:<name_w$}  {:<bin_w$}  {:<4}  {:<30}  {}\r\n",
                cursor_mark,
                sel_mark,
                check.name,
                check.bin_name,
                speed,
                patterns,
                action,
                name_w = name_w,
                bin_w = bin_w,
            )?;
            lines += 1;
        }
    }

    write!(
        stdout,
        "\r\n  ↑↓ navigate   space toggle   enter apply   q abort\r\n"
    )?;
    lines += 2;
    stdout.flush()?;
    Ok(lines)
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
