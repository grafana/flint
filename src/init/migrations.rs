use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::linters::typos::MigrationResult as TyposMigrationResult;
use crate::registry::{Check, EditorconfigDirectiveStyle, EditorconfigLineLengthPolicy, builtin};

use super::config_files::{
    remove_legacy_lint_files, remove_stale_editorconfig_checker_directives,
    remove_stale_markdownlint_line_length_directives,
};
use super::detection::parse_tool_keys;
use super::generation;
use super::generation::needs_node_for_npm;
use super::{ensure_node_for_npm, install_key, remove_tool_keys};

pub(super) struct RepoMigrationSummary {
    replaced_obsolete: Vec<(String, String)>,
    removed_unsupported: Vec<String>,
    node_added: bool,
    legacy_files_removed: Vec<String>,
    stale_md013_comments_removed: Vec<String>,
    stale_editorconfig_checker_comments_removed: Vec<String>,
    typos_migration: TyposMigrationResult,
}

struct MigrationInputs {
    tool_keys: HashSet<String>,
    mise_content: String,
}

impl RepoMigrationSummary {
    pub(super) fn noop() -> Self {
        Self {
            replaced_obsolete: vec![],
            removed_unsupported: vec![],
            node_added: false,
            legacy_files_removed: vec![],
            stale_md013_comments_removed: vec![],
            stale_editorconfig_checker_comments_removed: vec![],
            typos_migration: TyposMigrationResult::default(),
        }
    }

    pub(super) fn is_noop(&self) -> bool {
        self.replaced_obsolete.is_empty()
            && self.removed_unsupported.is_empty()
            && !self.node_added
            && self.legacy_files_removed.is_empty()
            && self.stale_md013_comments_removed.is_empty()
            && self.stale_editorconfig_checker_comments_removed.is_empty()
            && !self.typos_migration.changed()
    }

    pub(super) fn print_messages(&self) {
        for (old, new) in &self.replaced_obsolete {
            println!("  replaced {old:?} → {new:?}");
        }
        for old_key in &self.removed_unsupported {
            println!("  removed unsupported legacy linter {old_key:?}");
        }
        if self.node_added {
            println!("  added node (LTS) — required by npm: backend tools");
        }
        for rel in &self.legacy_files_removed {
            println!("  removed <REPO>/{rel} (legacy flint file)");
        }
        for rel in &self.stale_md013_comments_removed {
            println!("  removed stale markdownlint MD013 directives from <REPO>/{rel}");
        }
        for rel in &self.stale_editorconfig_checker_comments_removed {
            println!("  removed stale editorconfig-checker directives from <REPO>/{rel}");
        }
        self.typos_migration.print_messages();
    }
}

pub(crate) fn apply_setup_migrations(project_root: &Path, config_dir: &Path) -> Result<bool> {
    let inputs = migration_inputs(project_root)?;
    let obsolete_keys = crate::registry::obsolete_keys();
    let unsupported_keys = crate::registry::unsupported_keys();
    let delegated_sections = if legacy_markdownlint_stack_active(&inputs.tool_keys) {
        active_editorconfig_cleanup_sections(&inputs.tool_keys)
    } else {
        vec![]
    };
    let migration_summary = apply_repo_migrations_with_keys(
        project_root,
        config_dir,
        &delegated_sections,
        &obsolete_keys,
        &unsupported_keys,
        legacy_markdownlint_stack_active(&inputs.tool_keys),
    )?;
    migration_summary.print_messages();
    Ok(!migration_summary.is_noop())
}

pub(crate) fn detect_setup_migrations(project_root: &Path) -> Result<bool> {
    let inputs = migration_inputs(project_root)?;
    let obsolete_keys = crate::registry::obsolete_keys();
    let unsupported_keys = crate::registry::unsupported_keys();
    let mut migration_summary = detect_setup_migrations_with_keys(
        &obsolete_keys,
        &unsupported_keys,
        &inputs.tool_keys,
        &inputs.mise_content,
    );
    if crate::linters::typos::legacy_config_present(project_root) {
        migration_summary.typos_migration.wrote_target = true;
    }
    Ok(!migration_summary.is_noop())
}

pub(super) fn active_editorconfig_cleanup_sections(
    tool_keys: &HashSet<String>,
) -> Vec<(&'static [&'static str], EditorconfigDirectiveStyle)> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for check in builtin() {
        let Some(key) = install_key(&check) else {
            continue;
        };
        if !tool_keys.contains(key) {
            continue;
        }
        let EditorconfigLineLengthPolicy::DisableForPatterns {
            patterns,
            directive_style,
            ..
        } = check.editorconfig_line_length_policy
        else {
            continue;
        };
        let Some(EditorconfigDirectiveStyle::Html) = directive_style else {
            continue;
        };
        let dedupe_key = patterns.join(",");
        if seen.insert(dedupe_key) {
            out.push((patterns, EditorconfigDirectiveStyle::Html));
        }
    }
    out
}

pub(super) fn selected_editorconfig_line_length_sections(
    checks: &[&Check],
) -> Vec<(&'static [&'static str], &'static str)> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for check in checks {
        let EditorconfigLineLengthPolicy::DisableForPatterns {
            patterns, comment, ..
        } = check.editorconfig_line_length_policy
        else {
            continue;
        };
        let key = patterns.join(",");
        if seen.insert(key) {
            out.push((patterns, comment));
        }
    }
    out
}

pub(super) fn selected_editorconfig_cleanup_sections(
    checks: &[&Check],
) -> Vec<(&'static [&'static str], EditorconfigDirectiveStyle)> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for check in checks {
        let EditorconfigLineLengthPolicy::DisableForPatterns {
            patterns,
            directive_style,
            ..
        } = check.editorconfig_line_length_policy
        else {
            continue;
        };
        let Some(EditorconfigDirectiveStyle::Html) = directive_style else {
            continue;
        };
        let key = patterns.join(",");
        if seen.insert(key) {
            out.push((patterns, EditorconfigDirectiveStyle::Html));
        }
    }
    out
}

pub(super) fn apply_repo_migrations(
    project_root: &Path,
    config_dir: &Path,
    delegated_sections: &[(&'static [&'static str], EditorconfigDirectiveStyle)],
) -> Result<RepoMigrationSummary> {
    let obsolete_keys = crate::registry::obsolete_keys();
    let unsupported_keys = crate::registry::unsupported_keys();
    apply_repo_migrations_with_keys(
        project_root,
        config_dir,
        delegated_sections,
        &obsolete_keys,
        &unsupported_keys,
        true,
    )
}

fn migration_inputs(project_root: &Path) -> Result<MigrationInputs> {
    let mise_path = project_root.join("mise.toml");
    let current_content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    let current_tool_keys = parse_tool_keys(&current_content);
    Ok(MigrationInputs {
        tool_keys: current_tool_keys,
        mise_content: current_content,
    })
}

fn detect_setup_migrations_with_keys(
    obsolete_keys: &[(&'static str, &'static str)],
    unsupported_keys: &[(&'static str, &'static str)],
    tool_keys: &HashSet<String>,
    mise_content: &str,
) -> RepoMigrationSummary {
    let replaced_obsolete = obsolete_keys
        .iter()
        .filter(|(old_key, _)| tool_keys.contains(*old_key))
        .map(|(old_key, new_key)| ((*old_key).to_string(), (*new_key).to_string()))
        .collect();
    let removed_unsupported = unsupported_keys
        .iter()
        .filter(|(old_key, _)| tool_keys.contains(*old_key))
        .map(|(old_key, _)| (*old_key).to_string())
        .collect();
    let node_added = needs_node_for_npm(mise_content);
    RepoMigrationSummary {
        replaced_obsolete,
        removed_unsupported,
        node_added,
        legacy_files_removed: vec![],
        stale_md013_comments_removed: vec![],
        stale_editorconfig_checker_comments_removed: vec![],
        typos_migration: TyposMigrationResult::default(),
    }
}

fn apply_repo_migrations_with_keys(
    project_root: &Path,
    config_dir: &Path,
    delegated_sections: &[(&'static [&'static str], EditorconfigDirectiveStyle)],
    obsolete_keys: &[(&'static str, &'static str)],
    unsupported_keys: &[(&'static str, &'static str)],
    include_repo_cleanup: bool,
) -> Result<RepoMigrationSummary> {
    let replaced_obsolete =
        replace_obsolete_keys_preserving_decorations(project_root, obsolete_keys)?;
    let removed_unsupported = remove_tool_keys(
        project_root,
        &unsupported_keys
            .iter()
            .map(|(old_key, _)| *old_key)
            .collect::<Vec<_>>(),
    )?;
    let node_added = ensure_node_for_npm(project_root)?;
    let legacy_files_removed = if include_repo_cleanup {
        remove_legacy_lint_files(project_root)?
    } else {
        vec![]
    };
    let stale_md013_comments_removed =
        if include_repo_cleanup && delegated_patterns_include(delegated_sections, "*.md") {
            remove_stale_markdownlint_line_length_directives(project_root)?
        } else {
            vec![]
        };
    let stale_editorconfig_checker_comments_removed =
        if include_repo_cleanup && !delegated_sections.is_empty() {
            remove_stale_editorconfig_checker_directives(project_root, delegated_sections)?
        } else {
            vec![]
        };
    let typos_migration = crate::linters::typos::migrate_legacy_config(project_root, config_dir)?;

    Ok(RepoMigrationSummary {
        replaced_obsolete,
        removed_unsupported,
        node_added,
        legacy_files_removed,
        stale_md013_comments_removed,
        stale_editorconfig_checker_comments_removed,
        typos_migration,
    })
}

#[derive(Default)]
struct ToolDecorations {
    key_prefix: Option<String>,
    value_suffix: Option<String>,
}

/// Runs the mise key conversion and restores comments attached to the old key.
///
/// `toml_edit::Table::remove` returns the value item, but not the key's
/// decoration. The subsequent `Table::insert` therefore preserves an inline
/// value comment in most cases while dropping a reviewer-facing comment above
/// the key. Capture both sides before the conversion and restore them on the
/// replacement key. This intentionally stays scoped to obsolete-key
/// conversions; unrelated setup rewrites remain owned by their existing
/// migration helpers.
fn replace_obsolete_keys_preserving_decorations(
    project_root: &Path,
    obsolete: &[(&str, &str)],
) -> Result<Vec<(String, String)>> {
    let path = project_root.join("mise.toml");
    let before = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return generation::replace_obsolete_keys(project_root, obsolete);
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };

    let keys = obsolete
        .iter()
        .flat_map(|(old_key, new_key)| [*old_key, *new_key])
        .collect::<Vec<_>>();
    let decorations = capture_tool_decorations(&before, &keys);
    let replaced = generation::replace_obsolete_keys(project_root, obsolete)?;
    if replaced.is_empty() || decorations.is_empty() {
        return Ok(replaced);
    }

    restore_tool_decorations(&path, &decorations, &replaced)?;
    Ok(replaced)
}

fn capture_tool_decorations(content: &str, keys: &[&str]) -> HashMap<String, ToolDecorations> {
    let Ok(doc) = content.parse::<toml_edit::DocumentMut>() else {
        return HashMap::new();
    };
    let Some(tools) = doc.get("tools").and_then(|item| item.as_table()) else {
        return HashMap::new();
    };

    keys.iter()
        .filter_map(|key| {
            let item = tools.get(key)?;
            let key_prefix = tools
                .key(key)
                .and_then(|key| key.leaf_decor().prefix())
                .and_then(|raw| raw_string_text(raw, content));
            let value_suffix = item
                .as_value()
                .and_then(|value| value.decor().suffix())
                .and_then(|raw| raw_string_text(raw, content));
            Some((
                (*key).to_string(),
                ToolDecorations {
                    key_prefix,
                    value_suffix,
                },
            ))
        })
        .collect()
}

fn raw_string_text(raw: &toml_edit::RawString, source: &str) -> Option<String> {
    raw.as_str().map(str::to_string).or_else(|| {
        raw.span()
            .and_then(|span| source.get(span).map(str::to_string))
    })
}

fn restore_tool_decorations(
    path: &Path,
    decorations: &HashMap<String, ToolDecorations>,
    replaced: &[(String, String)],
) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {} after migration", path.display()))?;
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .with_context(|| format!("failed to parse {} after migration", path.display()))?;
    let Some(tools) = doc.get_mut("tools").and_then(|item| item.as_table_mut()) else {
        return Ok(());
    };

    let mut changed = false;
    for (old_key, new_key) in replaced {
        let Some(source) = decorations.get(old_key) else {
            continue;
        };
        {
            let Some(mut key) = tools.key_mut(new_key) else {
                continue;
            };
            if let Some(prefix) = source.key_prefix.as_deref()
                && prefix.contains('#')
            {
                key.leaf_decor_mut().set_prefix(prefix);
                changed = true;
            }
        }
        if let Some(suffix) = source.value_suffix.as_deref()
            && suffix.contains('#')
            && let Some(value) = tools.get_mut(new_key).and_then(|item| item.as_value_mut())
        {
            value.decor_mut().set_suffix(suffix);
            changed = true;
        }
    }

    if changed {
        std::fs::write(path, doc.to_string())
            .with_context(|| format!("failed to write {} after migration", path.display()))?;
    }
    Ok(())
}

fn legacy_markdownlint_stack_active(tool_keys: &HashSet<String>) -> bool {
    const MARKDOWNLINT_STACK_KEYS: &[&str] = &[
        "npm:markdownlint-cli",
        "npm:markdownlint-cli2",
        "npm:prettier",
    ];

    MARKDOWNLINT_STACK_KEYS
        .iter()
        .any(|key| tool_keys.contains(*key))
}

fn delegated_patterns_include(
    delegated_sections: &[(&'static [&'static str], EditorconfigDirectiveStyle)],
    needle: &str,
) -> bool {
    delegated_sections
        .iter()
        .any(|(patterns, _)| patterns.contains(&needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obsolete_key_conversion_restores_key_and_inline_comments() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("mise.toml");
        let before = "[tools]\n# Keep this pin until the replacement is verified.\nold = \"1.2.3\" # tracked by release engineering\n";
        std::fs::write(&path, before).unwrap();

        let replaced =
            replace_obsolete_keys_preserving_decorations(temp_dir.path(), &[("old", "new")])
                .unwrap();
        assert_eq!(replaced, [("old".to_string(), "new".to_string())]);

        let migrated = std::fs::read_to_string(path).unwrap();
        assert!(migrated.contains("# Keep this pin until the replacement is verified."));
        assert!(migrated.contains("# tracked by release engineering"));
    }
}
