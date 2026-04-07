use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
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
type DesiredTools = HashMap<String, Option<String>>;

// One entry per install key — groups all checks sharing that key.
struct LinterGroup<'a> {
    key: &'static str,
    checks: Vec<&'a Check>,    // sorted by name
    check_selected: Vec<bool>, // parallel to checks
    installed: bool,
    current_components: Option<String>,
}

impl LinterGroup<'_> {
    fn any_selected(&self) -> bool {
        self.check_selected.iter().any(|&s| s)
    }

    /// Components string to write for the currently selected checks, e.g. `"clippy,rustfmt"`.
    /// Returns `None` when no selected check carries a component requirement.
    fn selected_components(&self) -> Option<String> {
        let comps: Vec<&'static str> = self
            .checks
            .iter()
            .zip(&self.check_selected)
            .filter_map(|(c, &sel)| if sel { c.mise_install_components } else { None })
            .collect();
        if comps.is_empty() {
            None
        } else {
            Some(comps.join(","))
        }
    }

    fn action(&self) -> &'static str {
        if self.any_selected() {
            if !self.installed {
                "add"
            } else if self.selected_components() != self.current_components {
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
    let mut final_add: Vec<(String, Option<String>)> = Vec::new();
    let mut final_remove: Vec<String> = Vec::new();
    let mut final_upgrade: Vec<(String, String)> = Vec::new();

    for group in &groups {
        if group.any_selected() {
            if !group.installed {
                final_add.push((group.key.to_string(), group.selected_components()));
            } else {
                let target = group.selected_components();
                if target != group.current_components {
                    // Upgrade: components changed (added, removed, or reordered).
                    // If the target has no components (e.g. all component-bearing checks
                    // deselected), treat as a plain-version install via add+remove.
                    if let Some(comps) = target {
                        final_upgrade.push((group.key.to_string(), comps));
                    }
                }
            }
        } else if group.installed && known_keys.contains(group.key) {
            final_remove.push(group.key.to_string());
        }
    }

    let has_slow = has_slow_selected(&groups);
    let has_renovate = groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.name == "renovate-deps")
    });

    // Prompt for the flint config dir (skipped if already set in mise.toml or --yes).
    let existing_config_dir = get_existing_config_dir(&current_content);
    let config_dir_rel = prompt_config_dir(existing_config_dir.as_deref(), yes)?;

    let tools_changed =
        !final_add.is_empty() || !final_remove.is_empty() || !final_upgrade.is_empty();
    if tools_changed {
        apply_changes(
            &mise_path,
            &current_content,
            &final_add,
            &final_remove,
            &final_upgrade,
        )?;
    }

    let meta_changed = apply_env_and_tasks(&mise_path, &config_dir_rel, has_slow)?;

    let base_branch = detect_base_branch(project_root);
    let config_dir_path = project_root.join(&config_dir_rel);
    let toml_generated = generate_flint_toml(&config_dir_path, &base_branch, has_renovate)?;
    let workflow_generated = generate_lint_workflow(project_root, &base_branch)?;

    if !tools_changed && !meta_changed && !toml_generated && !workflow_generated {
        println!("No changes to apply.");
        return Ok(());
    }

    let hook_task = if has_slow { "lint:pre-commit" } else { "lint" };
    maybe_install_hook(project_root, hook_task, yes)?;

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
    // Collect per-key component lists so multiple checks sharing a key are merged.
    let mut by_key: HashMap<String, Vec<&'static str>> = HashMap::new();
    for check in registry {
        let key = match install_key(check) {
            Some(k) => k,
            None => continue,
        };
        if !files_present(check, present_patterns) {
            continue;
        }
        if categories.contains(&check.category) {
            let entry = by_key.entry(key.to_string()).or_default();
            if let Some(comp) = check.mise_install_components {
                if !entry.contains(&comp) {
                    entry.push(comp);
                }
            }
        }
    }
    by_key
        .into_iter()
        .map(|(k, comps)| {
            let merged = if comps.is_empty() {
                None
            } else {
                Some(comps.join(","))
            };
            (k, merged)
        })
        .collect()
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
#[cfg(test)]
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

/// Returns the `components` string currently set for `key` in the `[tools]` section,
/// or `None` if the key is absent, is a plain string entry, or has no `components` field.
fn get_entry_components(content: &str, key: &str) -> Option<String> {
    let doc: toml_edit::DocumentMut = content.parse().ok()?;
    let tools = doc.get("tools")?.as_table()?;
    match tools.get(key)?.as_value()? {
        toml_edit::Value::InlineTable(tbl) => tbl.get("components")?.as_str().map(str::to_string),
        _ => None,
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
            let current_components = if installed {
                get_entry_components(current_content, key)
            } else {
                None
            };
            // Pre-select each check individually: select if its category is in the
            // default set and its patterns are present, OR if the key is already installed.
            let check_selected: Vec<bool> = checks
                .iter()
                .map(|c| {
                    let suggested = default_categories.contains(&c.category)
                        && files_present(c, present_patterns);
                    suggested || installed
                })
                .collect();
            LinterGroup {
                key,
                checks,
                check_selected,
                installed,
                current_components,
            }
        })
        .collect();

    groups.sort_by_key(|g| g.checks.first().map_or(g.key, |c| c.name));
    groups
}

fn run_arrow_selector<T>(
    items: &mut [T],
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

fn select_categories_arrow(items: &mut [CategoryItem]) -> Result<bool> {
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

/// Maps a flat row index (across all checks in all groups) to `(group_idx, check_idx)`.
fn flat_to_group_check(groups: &[LinterGroup], flat: usize) -> (usize, usize) {
    let mut remaining = flat;
    for (gi, group) in groups.iter().enumerate() {
        if remaining < group.checks.len() {
            return (gi, remaining);
        }
        remaining -= group.checks.len();
    }
    (0, 0)
}

fn interactive_select_linters(groups: &mut Vec<LinterGroup>) -> Result<bool> {
    let total_rows = |gs: &[LinterGroup]| gs.iter().map(|g| g.checks.len()).sum::<usize>();
    let mut cursor = 0usize;
    terminal::enable_raw_mode()?;
    let result = (|| -> Result<bool> {
        let mut stdout = io::stdout();
        let mut n_lines = print_linter_table(&mut stdout, groups, cursor)?;
        loop {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Up if cursor > 0 => cursor -= 1,
                    KeyCode::Down if cursor + 1 < total_rows(groups) => cursor += 1,
                    KeyCode::Char(' ') => {
                        let (gi, ci) = flat_to_group_check(groups, cursor);
                        groups[gi].check_selected[ci] = !groups[gi].check_selected[ci];
                    }
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
                n_lines = print_linter_table(&mut stdout, groups, cursor)?;
            }
        }
    })();
    let _ = terminal::disable_raw_mode();
    println!();
    result
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

    let mut flat_idx = 0usize;
    for group in groups.iter() {
        let action = group.action();
        for (ci, check) in group.checks.iter().enumerate() {
            let sel_mark = if group.check_selected[ci] {
                "[✓]"
            } else {
                "[ ]"
            };
            let cursor_mark = if flat_idx == cursor { ">" } else { " " };
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
            flat_idx += 1;
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

// --- Post-linter-selection setup helpers ---

/// Returns true if any currently-selected check has `Category::Slow`.
fn has_slow_selected(groups: &[LinterGroup]) -> bool {
    groups.iter().any(|g| {
        g.checks
            .iter()
            .zip(&g.check_selected)
            .any(|(c, &sel)| sel && c.category == Category::Slow)
    })
}

/// Reads the default branch for `origin` from git, falling back to `"main"`.
fn detect_base_branch(project_root: &Path) -> String {
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
fn get_existing_config_dir(content: &str) -> Option<String> {
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
fn prompt_config_dir(existing: Option<&str>, yes: bool) -> Result<String> {
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
fn generate_flint_toml(config_dir: &Path, base_branch: &str, has_renovate: bool) -> Result<bool> {
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
fn generate_lint_workflow(project_root: &Path, base_branch: &str) -> Result<bool> {
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
fn apply_env_and_tasks(mise_path: &Path, config_dir_rel: &str, has_slow: bool) -> Result<bool> {
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
fn maybe_install_hook(project_root: &Path, hook_task: &str, yes: bool) -> Result<()> {
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
            &[("rust".to_string(), "clippy,rustfmt".to_string())],
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
        // Both cargo-clippy and cargo-fmt share the "rust" key; their components are merged.
        assert_eq!(
            tools.get("rust"),
            Some(&Some("clippy,rustfmt".to_string())),
            "rust tool entry should carry merged components"
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

    #[test]
    fn has_slow_selected_detects_slow_check() {
        let registry = builtin();
        let mut present = HashSet::new();
        present.insert(".github/renovate.json5".to_string());
        let categories = profile_to_categories(Profile::Comprehensive);
        let groups = build_linter_groups(&registry, &present, &HashSet::new(), "", &categories);
        assert!(has_slow_selected(&groups));
    }

    #[test]
    fn has_slow_selected_false_for_default_profile() {
        let registry = builtin();
        let present = HashSet::new();
        let categories = profile_to_categories(Profile::Default);
        let groups = build_linter_groups(&registry, &present, &HashSet::new(), "", &categories);
        assert!(!has_slow_selected(&groups));
    }

    #[test]
    fn get_existing_config_dir_reads_env_section() {
        let content = "[env]\nFLINT_CONFIG_DIR = \".github/config\"\n";
        assert_eq!(
            get_existing_config_dir(content),
            Some(".github/config".to_string())
        );
    }

    #[test]
    fn get_existing_config_dir_absent() {
        let content = "[tools]\nrust = \"latest\"\n";
        assert_eq!(get_existing_config_dir(content), None);
    }

    #[test]
    fn generate_flint_toml_writes_skeleton() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("config");
        let written = generate_flint_toml(&dir, "main", false).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(dir.join("flint.toml")).unwrap();
        assert!(content.contains("[settings]"));
        assert!(content.contains("# exclude ="));
        assert!(content.contains("# exclude_paths ="));
        assert!(!content.contains("base_branch")); // "main" is the default, omitted
    }

    #[test]
    fn generate_flint_toml_non_main_branch() {
        let tmp = tempfile::TempDir::new().unwrap();
        let written = generate_flint_toml(tmp.path(), "master", false).unwrap();
        assert!(written);
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
        assert!(content.contains("base_branch = \"master\""));
    }

    #[test]
    fn generate_flint_toml_with_renovate() {
        let tmp = tempfile::TempDir::new().unwrap();
        generate_flint_toml(tmp.path(), "main", true).unwrap();
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
        assert!(content.contains("[checks.renovate-deps]"));
        assert!(content.contains("# exclude_managers ="));
    }

    #[test]
    fn generate_flint_toml_skips_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("flint.toml"), "existing content").unwrap();
        let written = generate_flint_toml(tmp.path(), "main", false).unwrap();
        assert!(!written);
        let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
        assert_eq!(content, "existing content");
    }

    #[test]
    fn generate_lint_workflow_writes_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let written = generate_lint_workflow(tmp.path(), "main").unwrap();
        assert!(written);
        let content =
            std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
        assert!(content.contains("branches: [main]"));
        assert!(content.contains("mise run lint"));
        assert!(content.contains("fetch-depth: 0"));
        assert!(content.contains("persist-credentials: false"));
        assert!(content.contains("mise-action"));
        assert!(content.contains("github.token"));
    }

    #[test]
    fn generate_lint_workflow_non_main_branch() {
        let tmp = tempfile::TempDir::new().unwrap();
        generate_lint_workflow(tmp.path(), "master").unwrap();
        let content =
            std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
        assert!(content.contains("branches: [master]"));
    }

    #[test]
    fn generate_lint_workflow_skips_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
        std::fs::write(
            tmp.path().join(".github/workflows/lint.yml"),
            "existing content",
        )
        .unwrap();
        let written = generate_lint_workflow(tmp.path(), "main").unwrap();
        assert!(!written);
        let content =
            std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
        assert_eq!(content, "existing content");
    }

    #[test]
    fn apply_env_and_tasks_adds_sections() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "[tools]\nrust = \"latest\"\n").unwrap();
        let changed = apply_env_and_tasks(tmp.path(), ".github/config", false).unwrap();
        assert!(changed);
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("FLINT_CONFIG_DIR = \".github/config\""));
        assert!(content.contains("flint run"));
        assert!(content.contains("flint run --fix"));
        assert!(!content.contains("--fast-only")); // no slow linters
        assert!(content.contains("setup:pre-commit-hook"));
    }

    #[test]
    fn apply_env_and_tasks_adds_pre_commit_task_when_slow() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "").unwrap();
        apply_env_and_tasks(tmp.path(), ".", true).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("--fast-only"));
        assert!(content.contains("lint:pre-commit"));
        // Hook task should point to lint:pre-commit
        assert!(content.contains("--task=lint:pre-commit"));
    }

    #[test]
    fn apply_env_and_tasks_idempotent() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "").unwrap();
        apply_env_and_tasks(tmp.path(), ".github/config", false).unwrap();
        let after_first = std::fs::read_to_string(tmp.path()).unwrap();
        let changed = apply_env_and_tasks(tmp.path(), ".github/config", false).unwrap();
        assert!(!changed);
        let after_second = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(after_first, after_second);
    }
}
