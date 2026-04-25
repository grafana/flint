use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use crate::registry::{Category, Check, OBSOLETE_KEYS};

use super::{LinterGroup, install_key};

/// Returns `true` if the repo contains at least one file matching any of the
/// check's patterns. Checks with no patterns (project-scope specials like
/// lychee) are always considered present.
pub(super) fn files_present(check: &Check, present_patterns: &HashSet<String>) -> bool {
    check.patterns.is_empty()
        || check
            .patterns
            .iter()
            .any(|p| *p == "*" || present_patterns.contains(*p))
}

/// Runs `git ls-files -- <pattern>` for every unique pattern in the registry
/// and returns the set of patterns that produced at least one result.
pub(super) fn detect_present_patterns(
    project_root: &Path,
    registry: &[Check],
) -> Result<HashSet<String>> {
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
pub(super) fn parse_tool_keys(content: &str) -> HashSet<String> {
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
pub(super) fn entry_components_differ(content: &str, key: &str, required: &str) -> bool {
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
pub(super) fn get_entry_components(content: &str, key: &str) -> Option<String> {
    let doc: toml_edit::DocumentMut = content.parse().ok()?;
    let tools = doc.get("tools")?.as_table()?;
    match tools.get(key)?.as_value()? {
        toml_edit::Value::InlineTable(tbl) => tbl.get("components")?.as_str().map(str::to_string),
        _ => None,
    }
}

/// Returns the subset of `OBSOLETE_KEYS` whose old key is present in `current_tool_keys`.
pub(super) fn detect_obsolete_keys(
    current_tool_keys: &HashSet<String>,
) -> Vec<(&'static str, &'static str)> {
    OBSOLETE_KEYS
        .iter()
        .filter(|(old, _)| current_tool_keys.contains(*old))
        .copied()
        .collect()
}

/// Builds one `LinterGroup` per install key, covering all checks whose file patterns
/// are present in the repo or whose key is already installed.
pub(super) fn build_linter_groups<'a>(
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
            // Preselect each check individually: select if its category is in the
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
