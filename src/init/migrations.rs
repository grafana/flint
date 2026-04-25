use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

use crate::registry::{EditorconfigLineLengthPolicy, builtin};

use super::config_files::{
    remove_legacy_lint_files, remove_stale_editorconfig_checker_directives,
    remove_stale_markdownlint_line_length_directives,
};
use super::generation;
use super::{ensure_node_for_npm, install_key, remove_tool_keys};

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

pub(super) fn active_editorconfig_line_length_sections(
    tool_keys: &HashSet<String>,
) -> Vec<(&'static [&'static str], &'static str)> {
    let mut seen = HashSet::new();
    let mut out = vec![];
    for check in builtin() {
        let Some(key) = install_key(&check) else {
            continue;
        };
        if !tool_keys.contains(key) {
            continue;
        }
        let EditorconfigLineLengthPolicy::DisableForPatterns { patterns, comment } =
            check.editorconfig_line_length_policy
        else {
            continue;
        };
        let dedupe_key = patterns.join(",");
        if seen.insert(dedupe_key) {
            out.push((patterns, comment));
        }
    }
    out
}

pub(super) fn apply_repo_migrations(
    project_root: &Path,
    config_dir: &Path,
    delegated_patterns: &[&'static [&'static str]],
) -> Result<RepoMigrationSummary> {
    let obsolete_keys = crate::registry::obsolete_keys();
    let unsupported_keys = crate::registry::unsupported_keys();
    apply_repo_migrations_with_keys(
        project_root,
        config_dir,
        delegated_patterns,
        &obsolete_keys,
        &unsupported_keys,
    )
}

pub(super) fn apply_setup_migrations_after(
    project_root: &Path,
    config_dir: &Path,
    delegated_patterns: &[&'static [&'static str]],
    setup_version: u32,
) -> Result<RepoMigrationSummary> {
    let obsolete_keys = crate::setup::obsolete_keys_after(setup_version);
    let unsupported_keys = crate::setup::unsupported_keys_after(setup_version);
    apply_repo_migrations_with_keys(
        project_root,
        config_dir,
        delegated_patterns,
        &obsolete_keys,
        &unsupported_keys,
    )
}

fn apply_repo_migrations_with_keys(
    project_root: &Path,
    config_dir: &Path,
    delegated_patterns: &[&'static [&'static str]],
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
    let stale_md013_comments_removed = if delegated_patterns_include(delegated_patterns, "*.md") {
        remove_stale_markdownlint_line_length_directives(project_root)?
    } else {
        vec![]
    };
    let stale_editorconfig_checker_comments_removed = if delegated_patterns.is_empty() {
        vec![]
    } else {
        remove_stale_editorconfig_checker_directives(project_root, delegated_patterns)?
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
    delegated_patterns: &[&'static [&'static str]],
    needle: &str,
) -> bool {
    delegated_patterns
        .iter()
        .any(|patterns| patterns.contains(&needle))
}
