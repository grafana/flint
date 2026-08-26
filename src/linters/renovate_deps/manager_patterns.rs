//! Local (offline) relevance detection for `customManagers[].managerFilePatterns`.
//!
//! Renovate's own extraction is the source of truth, but running it is
//! expensive and CI-only for files that aren't yet tracked in the committed
//! snapshot. This module lets `is_relevant` catch the common case where a
//! *new* file matches a pattern that an *existing* custom manager already
//! declares, without shelling out to Renovate.
//!
//! Patterns declared inline in the active config are always resolvable.
//! Patterns that come from `extends` are resolved from local files, Flint's
//! bundled preset, or a cached GitHub preset. GitHub presets are fetched by
//! `flint init` when a token is available and cached in the same local cache
//! directory used by lychee. A normal `flint run` only reads that cache: it
//! never performs a network request. Cache-cold presets (and unsupported
//! `gitlab>`, npm, or HTTP presets) are silently skipped and fall back to the
//! existing CI-only detection.

use std::collections::HashSet;
use std::path::{Component, Path};
use std::process::Stdio;

use globset::GlobBuilder;
use regex::Regex;

const FLINT_DEFAULT_PRESET: &str = include_str!("../../../default.json");
const FLINT_PRESET_EXTEND_PREFIX: &str = "github>grafana/flint";
const GITHUB_PRESET_PREFIX: &str = "github>";
const PRESET_CACHE_SUBDIR: &str = "renovate-presets";
const MAX_PRESET_DEPTH: usize = 16;

#[derive(Debug, Clone)]
struct GithubPreset {
    owner: String,
    repo: String,
    path: String,
    reference: Option<String>,
}

pub(crate) fn bundled_extract_version_dep_names(
    project_root: &Path,
    config_content: &str,
) -> HashSet<String> {
    let mut visited = HashSet::new();
    if !extends_flint_preset(project_root, config_content, &mut visited, 0) {
        return HashSet::new();
    }

    serde_json::from_str::<serde_json::Value>(FLINT_DEFAULT_PRESET)
        .ok()
        .and_then(|preset| preset["packageRules"].as_array().cloned())
        .into_iter()
        .flatten()
        .filter(|rule| !rule["extractVersion"].is_null())
        .flat_map(|rule| {
            rule["matchDepNames"]
                .as_array()
                .cloned()
                .into_iter()
                .flatten()
        })
        .filter_map(|dep_name| dep_name.as_str().map(ToOwned::to_owned))
        .collect()
}

fn extends_flint_preset(
    project_root: &Path,
    config_content: &str,
    visited: &mut HashSet<String>,
    depth: usize,
) -> bool {
    if depth >= MAX_PRESET_DEPTH {
        return false;
    }
    let Ok(config) = json5::from_str::<serde_json::Value>(config_content) else {
        return false;
    };

    extends_entries(&config).into_iter().any(|entry| {
        is_flint_preset_extend(entry)
            || resolve_extend(project_root, entry, visited).is_some_and(|resolved| {
                extends_flint_preset(project_root, &resolved, visited, depth + 1)
            })
    })
}

/// Returns true if any `changed` path matches a `managerFilePatterns` entry
/// declared by `config_content` (or one of its resolvable `extends`).
pub(crate) fn changed_matches_manager_file_patterns(
    project_root: &Path,
    config_content: &str,
    changed: &HashSet<String>,
) -> bool {
    let mut visited = HashSet::new();
    let patterns = collect_patterns(project_root, config_content, &mut visited, 0);
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
    depth: usize,
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

    if depth >= MAX_PRESET_DEPTH {
        return patterns;
    }

    for entry in extends_entries(&parsed) {
        if let Some(resolved) = resolve_extend(project_root, entry, visited) {
            patterns.extend(collect_patterns(
                project_root,
                &resolved,
                visited,
                depth + 1,
            ));
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
/// offline. Entries that are unsupported, cache-cold, or already resolved
/// return `None`; recognized entries are recorded in `visited` before reading
/// so cycles cannot recurse indefinitely.
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

    if let Some(preset) = parse_github_preset(entry) {
        let key = github_preset_key(&preset);
        if !visited.insert(key) {
            return None;
        }
        return read_cached_github_preset(project_root, &preset);
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

/// Fetches cache-cold GitHub presets for `flint init`. This is intentionally
/// not called by [`changed_matches_manager_file_patterns`], since relevance
/// checks must remain offline and fast. Errors are ignored: a missing token,
/// unavailable network, or unsupported preset should preserve the existing
/// silent-skip behavior.
pub(crate) fn warm_github_presets(project_root: &Path, config_content: &str) {
    let mut visited = HashSet::new();
    warm_config(project_root, config_content, &mut visited, 0);
}

fn warm_config(
    project_root: &Path,
    config_content: &str,
    visited: &mut HashSet<String>,
    depth: usize,
) {
    if depth >= MAX_PRESET_DEPTH {
        return;
    }
    let Ok(parsed) = json5::from_str::<serde_json::Value>(config_content) else {
        return;
    };

    for entry in extends_entries(&parsed) {
        if is_flint_preset_extend(entry) {
            continue;
        }
        if let Some(preset) = parse_github_preset(entry) {
            let key = github_preset_key(&preset);
            if !visited.insert(key) {
                continue;
            }
            if !cached_github_preset_exists(project_root, &preset) {
                let _ = fetch_github_preset(project_root, &preset);
            }
            if let Some(resolved) = read_cached_github_preset(project_root, &preset) {
                warm_config(project_root, &resolved, visited, depth + 1);
            }
            continue;
        }

        if let Some(resolved) = resolve_extend(project_root, entry, visited) {
            warm_config(project_root, &resolved, visited, depth + 1);
        }
    }
}

fn parse_github_preset(entry: &str) -> Option<GithubPreset> {
    let rest = entry.strip_prefix(GITHUB_PRESET_PREFIX)?;
    let (rest, reference) = rest
        .split_once('#')
        .map_or((rest, None), |(value, reference)| (value, Some(reference)));
    let (repo_part, path) = rest
        .split_once("//")
        .map_or((rest, "default.json"), |(repo, path)| (repo, path));
    let mut repo_parts = repo_part.split('/');
    let owner = repo_parts.next()?;
    let repo = repo_parts.next()?;
    if repo_parts.next().is_some()
        || owner.is_empty()
        || repo.is_empty()
        || path.is_empty()
        || !valid_remote_component(owner)
        || !valid_remote_component(repo)
        || !path.split('/').all(valid_remote_component)
        || reference.is_some_and(|value| value.is_empty() || value.contains('\0'))
    {
        return None;
    }
    Some(GithubPreset {
        owner: owner.to_string(),
        repo: repo.to_string(),
        path: path.to_string(),
        reference: reference.map(ToOwned::to_owned),
    })
}

fn valid_remote_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('\0')
        && !value.contains('\\')
        && !value.contains(':')
}

fn github_preset_key(preset: &GithubPreset) -> String {
    format!(
        "github>{}/{}//{}#{}",
        preset.owner,
        preset.repo,
        preset.path,
        preset.reference.as_deref().unwrap_or("<default>")
    )
}

fn github_preset_cache_path(project_root: &Path, preset: &GithubPreset) -> std::path::PathBuf {
    let reference = preset.reference.as_deref().unwrap_or("<default>");
    let path = preset
        .path
        .split('/')
        .map(percent_encode)
        .collect::<Vec<_>>()
        .join("/");
    project_root
        .join(crate::linters::lychee::LOCAL_CACHE_DIR)
        .join(PRESET_CACHE_SUBDIR)
        .join(percent_encode(&preset.owner))
        .join(percent_encode(&preset.repo))
        .join(percent_encode(reference))
        .join(path)
}

fn cached_github_preset_exists(project_root: &Path, preset: &GithubPreset) -> bool {
    github_preset_cache_path(project_root, preset).is_file()
}

fn read_cached_github_preset(project_root: &Path, preset: &GithubPreset) -> Option<String> {
    std::fs::read_to_string(github_preset_cache_path(project_root, preset)).ok()
}

fn fetch_github_preset(project_root: &Path, preset: &GithubPreset) -> anyhow::Result<()> {
    let token = std::env::var(crate::linters::env::GITHUB_COM_TOKEN_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var(crate::linters::env::GITHUB_TOKEN_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
    let Some(token) = token else {
        return Ok(());
    };

    let api_url = std::env::var("GITHUB_API_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://api.github.com".to_string());
    let api_url = api_url.trim_end_matches('/');
    let mut url = format!(
        "{api_url}/repos/{}/{}/contents/{}",
        preset.owner,
        preset.repo,
        preset
            .path
            .split('/')
            .map(percent_encode)
            .collect::<Vec<_>>()
            .join("/")
    );
    if let Some(reference) = &preset.reference {
        url.push_str("?ref=");
        url.push_str(&percent_encode(reference));
    }

    let output = std::process::Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "10",
            "--header",
            "Accept: application/vnd.github.raw+json",
            "--header",
            "User-Agent: flint",
            "--header",
            &format!("Authorization: Bearer {token}"),
            &url,
        ])
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() || output.stdout.is_empty() {
        return Ok(());
    }

    let destination = github_preset_cache_path(project_root, preset);
    if let Some(parent) = destination.parent() {
        crate::linters::lychee::initialize_cache_dir(project_root)?;
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(destination, output.stdout)?;
    Ok(())
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            byte => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn cache_preset(root: &Path, entry: &str, content: &str) {
        let preset = parse_github_preset(entry).expect("valid GitHub preset");
        let path = github_preset_cache_path(root, &preset);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn parses_github_preset_forms() {
        let default = parse_github_preset("github>owner/repo").unwrap();
        assert_eq!(default.owner, "owner");
        assert_eq!(default.repo, "repo");
        assert_eq!(default.path, "default.json");
        assert_eq!(default.reference, None);

        let nested = parse_github_preset("github>owner/repo//presets/base.json#v2").unwrap();
        assert_eq!(nested.path, "presets/base.json");
        assert_eq!(nested.reference.as_deref(), Some("v2"));
    }

    #[test]
    fn cached_github_default_and_nested_presets_are_resolved() {
        let root = tempfile::tempdir().unwrap();
        cache_preset(
            root.path(),
            "github>owner/repo",
            r#"{ customManagers: [{ managerFilePatterns: ["**/*.yaml"] }] }"#,
        );
        cache_preset(
            root.path(),
            "github>owner/repo//presets/base.json#v2",
            r#"{ customManagers: [{ managerFilePatterns: ["**/*.toml"] }] }"#,
        );

        assert!(changed_matches_manager_file_patterns(
            root.path(),
            r#"{ extends: ["github>owner/repo"] }"#,
            &HashSet::from(["config/app.yaml".to_string()]),
        ));
        assert!(changed_matches_manager_file_patterns(
            root.path(),
            r#"{ extends: ["github>owner/repo//presets/base.json#v2"] }"#,
            &HashSet::from(["mise.toml".to_string()]),
        ));
    }

    #[test]
    fn cached_remote_preset_recursion_stops_on_cycle() {
        let root = tempfile::tempdir().unwrap();
        cache_preset(
            root.path(),
            "github>owner/one",
            r#"{ extends: ["github>owner/two"] }"#,
        );
        cache_preset(
            root.path(),
            "github>owner/two",
            r#"{ extends: ["github>owner/one"], customManagers: [{ managerFilePatterns: ["**/*.yml"] }] }"#,
        );

        assert!(changed_matches_manager_file_patterns(
            root.path(),
            r#"{ extends: ["github>owner/one"] }"#,
            &HashSet::from([".github/workflows/ci.yml".to_string()]),
        ));
    }

    #[cfg(unix)]
    #[test]
    fn warm_github_presets_fetches_cache_cold_and_skips_cache_hit() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::{Mutex, OnceLock};

        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let count = root.path().join("curl-count");
        let args = root.path().join("curl-args");
        let curl = bin.path().join("curl");
        std::fs::write(
            &curl,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nn=$(cat '{}' 2>/dev/null || printf 0)\nprintf '%s' $((n + 1)) > '{}'\nprintf '%s' '{{ customManagers: [{{ managerFilePatterns: [\\\"**/*.yaml\\\"] }}] }}'\n",
                args.display(),
                count.display(),
                count.display(),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();

        let old_path = std::env::var_os("PATH");
        let old_token = std::env::var_os(crate::linters::env::GITHUB_TOKEN_ENV);
        let old_api_url = std::env::var_os("GITHUB_API_URL");
        let mut path = bin.path().as_os_str().to_os_string();
        if let Some(old_path) = &old_path {
            path.push(":");
            path.push(old_path);
        }
        unsafe {
            std::env::set_var("PATH", path);
            std::env::set_var(crate::linters::env::GITHUB_TOKEN_ENV, "test-token");
            std::env::set_var("GITHUB_API_URL", "https://api.example.test");
        }

        let config = r#"{ extends: ["github>owner/repo//presets/base.json#v1"] }"#;
        warm_github_presets(root.path(), config);
        let preset = parse_github_preset("github>owner/repo//presets/base.json#v1").unwrap();
        assert!(github_preset_cache_path(root.path(), &preset).is_file());
        assert_eq!(std::fs::read_to_string(&count).unwrap(), "1");
        let request = std::fs::read_to_string(&args).unwrap();
        assert!(request.contains(
            "https://api.example.test/repos/owner/repo/contents/presets/base.json?ref=v1"
        ));

        // A warm cache is sufficient even with a token: no second API call is made.
        warm_github_presets(root.path(), config);
        assert_eq!(std::fs::read_to_string(&count).unwrap(), "1");

        unsafe {
            match old_path {
                Some(value) => std::env::set_var("PATH", value),
                None => std::env::remove_var("PATH"),
            }
            match old_token {
                Some(value) => std::env::set_var(crate::linters::env::GITHUB_TOKEN_ENV, value),
                None => std::env::remove_var(crate::linters::env::GITHUB_TOKEN_ENV),
            }
            match old_api_url {
                Some(value) => std::env::set_var("GITHUB_API_URL", value),
                None => std::env::remove_var("GITHUB_API_URL"),
            }
        }
    }
}
