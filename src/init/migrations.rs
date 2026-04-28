use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

use crate::registry::{Check, EditorconfigDirectiveStyle, EditorconfigLineLengthPolicy};

use super::config_files::{
    remove_legacy_lint_files, remove_stale_editorconfig_checker_directives,
    remove_stale_markdownlint_line_length_directives,
};
use super::detection::parse_tool_keys;
use super::generation;
use super::{ensure_node_for_npm, remove_tool_keys};

pub(super) struct RepoMigrationSummary {
    replaced_obsolete: Vec<(String, String)>,
    removed_unsupported: Vec<String>,
    node_added: bool,
    legacy_files_removed: Vec<String>,
    stale_md013_comments_removed: Vec<String>,
    stale_editorconfig_checker_comments_removed: Vec<String>,
}

impl RepoMigrationSummary {
    pub(super) fn is_noop(&self) -> bool {
        self.replaced_obsolete.is_empty()
            && self.removed_unsupported.is_empty()
            && !self.node_added
            && self.legacy_files_removed.is_empty()
            && self.stale_md013_comments_removed.is_empty()
            && self.stale_editorconfig_checker_comments_removed.is_empty()
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
            println!("  removed <REPO>/{rel} (legacy flint v1 / super-linter file)");
        }
        for rel in &self.stale_md013_comments_removed {
            println!("  removed stale markdownlint MD013 directives from <REPO>/{rel}");
        }
        for rel in &self.stale_editorconfig_checker_comments_removed {
            println!("  removed stale editorconfig-checker directives from <REPO>/{rel}");
        }
    }
}

pub(crate) fn apply_setup_migrations(project_root: &Path, _config_dir: &Path) -> Result<bool> {
    let obsolete_keys = crate::registry::obsolete_keys();
    let unsupported_keys = crate::registry::unsupported_keys();
    let replaced_obsolete = generation::replace_obsolete_keys(project_root, &obsolete_keys)?;
    let removed_unsupported = remove_tool_keys(
        project_root,
        &unsupported_keys
            .iter()
            .map(|(old_key, _)| *old_key)
            .collect::<Vec<_>>(),
    )?;
    Ok(!replaced_obsolete.is_empty() || !removed_unsupported.is_empty())
}

pub(crate) fn detect_setup_migrations(
    project_root: &Path,
    _config_dir: &Path,
    setup_migration_version: u32,
) -> Result<bool> {
    let mise_path = project_root.join("mise.toml");
    let current_content = std::fs::read_to_string(&mise_path).unwrap_or_default();
    let current_tool_keys = parse_tool_keys(&current_content);
    let obsolete_keys = crate::registry::obsolete_keys_after(setup_migration_version);
    let unsupported_keys = crate::setup::unsupported_keys_after(setup_migration_version);

    Ok(obsolete_keys
        .iter()
        .any(|(old_key, _)| current_tool_keys.contains(*old_key))
        || unsupported_keys
            .iter()
            .any(|(old_key, _)| current_tool_keys.contains(*old_key)))
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
    )
}

fn apply_repo_migrations_with_keys(
    project_root: &Path,
    config_dir: &Path,
    delegated_sections: &[(&'static [&'static str], EditorconfigDirectiveStyle)],
    obsolete_keys: &[(&'static str, &'static str)],
    unsupported_keys: &[(&'static str, &'static str)],
) -> Result<RepoMigrationSummary> {
    let replaced_obsolete = generation::replace_obsolete_keys(project_root, obsolete_keys)?;
    let removed_unsupported = remove_tool_keys(
        project_root,
        &unsupported_keys
            .iter()
            .map(|(old_key, _)| *old_key)
            .collect::<Vec<_>>(),
    )?;
    let node_added = ensure_node_for_npm(project_root)?;
    let legacy_files_removed = remove_legacy_lint_files(project_root, config_dir)?;
    let stale_md013_comments_removed = if delegated_patterns_include(delegated_sections, "*.md") {
        remove_stale_markdownlint_line_length_directives(project_root)?
    } else {
        vec![]
    };
    let stale_editorconfig_checker_comments_removed = if delegated_sections.is_empty() {
        vec![]
    } else {
        remove_stale_editorconfig_checker_directives(project_root, delegated_sections)?
    };

    Ok(RepoMigrationSummary {
        replaced_obsolete,
        removed_unsupported,
        node_added,
        legacy_files_removed,
        stale_md013_comments_removed,
        stale_editorconfig_checker_comments_removed,
    })
}

fn delegated_patterns_include(
    delegated_sections: &[(&'static [&'static str], EditorconfigDirectiveStyle)],
    needle: &str,
) -> bool {
    delegated_sections
        .iter()
        .any(|(patterns, _)| patterns.contains(&needle))
}
