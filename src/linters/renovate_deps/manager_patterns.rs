//! Local (offline) relevance detection for `customManagers[].managerFilePatterns`.
//!
//! Renovate's own extraction is the source of truth, but running it is
//! expensive and CI-only for files that aren't yet tracked in the committed
//! snapshot. This module lets `is_relevant` catch the common case where a
//! *new* file matches a pattern that an *existing* custom manager already
//! declares, without shelling out to Renovate.
//!
//! Patterns declared inline in the active config are always resolvable.
//! Patterns that come from `extends` are only resolvable when:
//! - the extend points at another file in the repo (`local>path` or a plain
//!   relative path), or
//! - the extend is `github>grafana/flint` (any/no version pin) — flint
//!   bundles its own preset (`default.json`) at compile time, since it's the
//!   one shipping that preset in the first place.
//!
//! Any other remote preset can't be resolved offline and is silently
//! skipped; those changes fall back to the existing CI-only detection.

use std::collections::HashSet;
use std::path::{Component, Path};

use globset::GlobBuilder;
use regex::Regex;

const FLINT_DEFAULT_PRESET: &str = include_str!("../../../default.json");
const FLINT_PRESET_EXTEND_PREFIX: &str = "github>grafana/flint";

/// Returns true if any `changed` path matches a `managerFilePatterns` entry
/// declared by `config_content` (or one of its resolvable `extends`).
pub(crate) fn changed_matches_manager_file_patterns(
    project_root: &Path,
    config_content: &str,
    changed: &HashSet<String>,
) -> bool {
    let mut visited = HashSet::new();
    let patterns = collect_patterns(project_root, config_content, &mut visited);
    patterns
        .iter()
        .any(|pattern| changed.iter().any(|path| pattern.is_match(path)))
}

enum CompiledPattern {
    Regex(Regex),
    Glob(globset::GlobMatcher),
}

impl CompiledPattern {
    fn is_match(&self, path: &str) -> bool {
        match self {
            CompiledPattern::Regex(re) => re.is_match(path),
            CompiledPattern::Glob(glob) => glob.is_match(path),
        }
    }
}

/// Renovate accepts either a `/regex/flags` string or a plain glob for
/// `managerFilePatterns` entries.
fn compile_pattern(raw: &str) -> Option<CompiledPattern> {
    if let Some(body) = raw.strip_prefix('/') {
        let end = body.rfind('/')?;
        let (body, flags) = body.split_at(end);
        let flags = &flags[1..];
        let prefix = if flags.contains('i') { "(?i)" } else { "" };
        return Regex::new(&format!("{prefix}{body}"))
            .ok()
            .map(CompiledPattern::Regex);
    }
    GlobBuilder::new(raw)
        .literal_separator(false)
        .build()
        .ok()
        .map(|glob| CompiledPattern::Glob(glob.compile_matcher()))
}

fn collect_patterns(
    project_root: &Path,
    config_content: &str,
    visited: &mut HashSet<String>,
) -> Vec<CompiledPattern> {
    let mut patterns = Vec::new();
    let Ok(parsed) = json5::from_str::<serde_json::Value>(config_content) else {
        return patterns;
    };

    if let Some(managers) = parsed.get("customManagers").and_then(|v| v.as_array()) {
        for manager in managers {
            let Some(file_patterns) = manager
                .get("managerFilePatterns")
                .and_then(|v| v.as_array())
            else {
                continue;
            };
            for pattern in file_patterns {
                if let Some(pattern) = pattern.as_str().and_then(compile_pattern) {
                    patterns.push(pattern);
                }
            }
        }
    }

    for entry in extends_entries(&parsed) {
        if let Some(resolved) = resolve_extend(project_root, entry, visited) {
            patterns.extend(collect_patterns(project_root, &resolved, visited));
        }
    }

    patterns
}

fn extends_entries(parsed: &serde_json::Value) -> Vec<&str> {
    match parsed.get("extends") {
        Some(serde_json::Value::String(entry)) => vec![entry.as_str()],
        Some(serde_json::Value::Array(entries)) => {
            entries.iter().filter_map(|v| v.as_str()).collect()
        }
        _ => Vec::new(),
    }
}

/// Resolves an `extends` entry to its config content, if we can do so
/// offline. Returns `None` (and records nothing in `visited`) for entries we
/// can't or have already resolved.
fn resolve_extend(
    project_root: &Path,
    entry: &str,
    visited: &mut HashSet<String>,
) -> Option<String> {
    if is_flint_preset_extend(entry) {
        if !visited.insert(FLINT_PRESET_EXTEND_PREFIX.to_string()) {
            return None;
        }
        return Some(FLINT_DEFAULT_PRESET.to_string());
    }

    let rel_path = entry.strip_prefix("local>").unwrap_or(entry);
    if !is_safe_local_extend_path(rel_path) {
        return None;
    }

    let base = project_root.join(rel_path);
    let candidate = [
        base.clone(),
        base.with_extension("json5"),
        base.with_extension("json"),
    ]
    .into_iter()
    .find(|path| path.is_file())?;
    let key = candidate.to_string_lossy().into_owned();
    if !visited.insert(key) {
        return None;
    }
    std::fs::read_to_string(&candidate).ok()
}

fn is_safe_local_extend_path(rel_path: &str) -> bool {
    if rel_path.is_empty() || rel_path.contains(':') {
        return false;
    }

    let path = Path::new(rel_path);
    if path.is_absolute() {
        return false;
    }

    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn is_flint_preset_extend(entry: &str) -> bool {
    entry == FLINT_PRESET_EXTEND_PREFIX
        || entry
            .strip_prefix(FLINT_PRESET_EXTEND_PREFIX)
            .is_some_and(|rest| rest.starts_with('#'))
}
