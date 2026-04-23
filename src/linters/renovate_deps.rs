use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::config::RenovateDepsConfig;
use crate::files::FileList;
use crate::linters::LinterOutput;

const COMMITTED_FILE: &str = "renovate-tracked-deps.json";
pub(crate) const COMMITTED_PATHS: &[&str] = &[COMMITTED_FILE, ".github/renovate-tracked-deps.json"];
pub(crate) const RENOVATE_CONFIG_PATTERNS: &[&str] = &[
    "renovate.json",
    "renovate.json5",
    ".github/renovate.json",
    ".github/renovate.json5",
    ".renovaterc",
    ".renovaterc.json",
    ".renovaterc.json5",
];
const PACKAGE_FILES_MSG: &str = "Extracted dependencies";
const SKIP_REASONS: &[&str] = &["contains-variable", "invalid-value", "invalid-version"];

/// `{file_path: {manager: [dep_name, ...]}}` — all collections sorted.
type DepMap = BTreeMap<String, BTreeMap<String, Vec<String>>>;

pub async fn run(cfg: &RenovateDepsConfig, fix: bool, project_root: &Path) -> LinterOutput {
    match run_inner(cfg, fix, project_root).await {
        Ok(out) => out,
        Err(e) => LinterOutput::err(format!("flint: renovate-deps: {e}\n")),
    }
}

pub(crate) fn is_relevant(file_list: &FileList, project_root: &Path) -> bool {
    if file_list.full {
        return true;
    }

    let changed: HashSet<String> = file_list
        .files
        .iter()
        .filter_map(|path| {
            path.strip_prefix(project_root)
                .ok()
                .map(|rel| rel.to_string_lossy().into_owned())
        })
        .collect();

    if changed.is_empty() {
        return false;
    }

    if changed
        .iter()
        .any(|path| RENOVATE_CONFIG_PATTERNS.contains(&path.as_str()))
    {
        return true;
    }

    let committed_path = COMMITTED_PATHS
        .iter()
        .map(|path| project_root.join(path))
        .find(|path| path.exists());

    let Some(committed_path) = committed_path else {
        return false;
    };

    let committed_rel = display_path(project_root, &committed_path);
    if changed.contains(&committed_rel) {
        return true;
    }

    let committed: DepMap = match std::fs::read_to_string(&committed_path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
    {
        Some(committed) => committed,
        None => return true,
    };

    committed.keys().any(|path| changed.contains(path))
}

async fn run_inner(
    cfg: &RenovateDepsConfig,
    fix: bool,
    project_root: &Path,
) -> anyhow::Result<LinterOutput> {
    let config_path = resolve_renovate_config_path(project_root)?;
    let committed_path = committed_path_for_config(&config_path);
    let committed_display = display_path(project_root, &committed_path);

    // Renovate occasionally produces empty packageFiles on the first run (transient
    // network or registry issue). Retry up to 3 times with a short delay.
    let mut generated = DepMap::default();
    for attempt in 1..=3u32 {
        let log_bytes = run_renovate(project_root, &config_path).await?;
        generated = extract_deps(&log_bytes, &cfg.exclude_managers)?;
        if !generated.is_empty() || attempt == 3 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    if !committed_path.exists() {
        if fix {
            write_snapshot(&committed_path, &generated)?;
            return Ok(LinterOutput {
                ok: true,
                stdout: format!("{COMMITTED_FILE} has been created.\n").into_bytes(),
                stderr: vec![],
            });
        }
        return Ok(LinterOutput::err(format!(
            "ERROR: {committed_display} does not exist.\nRun `flint run --fix renovate-deps` to create it.\n"
        )));
    }

    let committed: DepMap = serde_json::from_str(&std::fs::read_to_string(&committed_path)?)?;

    if committed == generated {
        return Ok(LinterOutput {
            ok: true,
            stdout: format!("{COMMITTED_FILE} is up to date.\n").into_bytes(),
            stderr: vec![],
        });
    }

    let diff = unified_diff(&committed, &generated, &committed_display);

    if fix {
        write_snapshot(&committed_path, &generated)?;
        let mut stdout = diff.into_bytes();
        stdout.extend_from_slice(format!("{COMMITTED_FILE} has been updated.\n").as_bytes());
        return Ok(LinterOutput {
            ok: true,
            stdout,
            stderr: vec![],
        });
    }

    Ok(LinterOutput {
        ok: false,
        stdout: diff.into_bytes(),
        stderr: format!(
            "ERROR: {COMMITTED_FILE} is out of date.\nRun `flint run --fix renovate-deps` to update.\n"
        )
        .into_bytes(),
    })
}

/// Runs `renovate --platform=local` and returns the combined stdout+stderr log bytes.
async fn run_renovate(project_root: &Path, config_path: &Path) -> anyhow::Result<Vec<u8>> {
    // Forward env, setting Renovate-specific vars.
    let mut env: Vec<(String, String)> = std::env::vars().collect();
    // Override logging to get parseable JSON output.
    env.retain(|(k, _)| k != "LOG_LEVEL" && k != "LOG_FORMAT" && k != "RENOVATE_CONFIG_FILE");
    env.push(("LOG_LEVEL".into(), "debug".into()));
    env.push(("LOG_FORMAT".into(), "json".into()));
    env.push((
        "RENOVATE_CONFIG_FILE".into(),
        config_path.to_string_lossy().into_owned(),
    ));
    // Renovate uses GITHUB_COM_TOKEN for github.com API calls; fall back to GITHUB_TOKEN.
    let has_com_token = std::env::var("GITHUB_COM_TOKEN")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if !has_com_token
        && let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        env.push(("GITHUB_COM_TOKEN".into(), token));
    }

    let out = super::spawn_command(
        &[
            "renovate".to_string(),
            "--platform=local".to_string(),
            "--require-config=ignored".to_string(),
            "--dry-run=extract".to_string(),
        ],
        false,
    )
    .current_dir(project_root)
    .envs(env)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .await?;

    // Combine stdout+stderr: Renovate writes JSON log lines to stdout, but
    // some startup messages may appear on stderr.
    let mut combined = out.stdout;
    combined.extend_from_slice(&out.stderr);

    if !out.status.success() {
        let snippet = String::from_utf8_lossy(&combined);
        anyhow::bail!(
            "renovate exited with status {}: {}",
            out.status.code().unwrap_or(-1),
            snippet.lines().take(20).collect::<Vec<_>>().join("\n")
        );
    }

    Ok(combined)
}

fn resolve_renovate_config_path(project_root: &Path) -> anyhow::Result<PathBuf> {
    RENOVATE_CONFIG_PATTERNS
        .iter()
        .map(|path| project_root.join(path))
        .find(|path| path.exists())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no supported Renovate config file found; tried: {}",
                RENOVATE_CONFIG_PATTERNS.join(", ")
            )
        })
}

fn committed_path_for_config(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(COMMITTED_FILE)
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Parses Renovate's NDJSON log and returns the dep map.
fn extract_deps(log_bytes: &[u8], exclude_managers: &[String]) -> anyhow::Result<DepMap> {
    let log = std::str::from_utf8(log_bytes)?;

    let exclude: HashSet<&str> = exclude_managers.iter().map(String::as_str).collect();

    // Find the last "packageFiles with updates" log entry — Renovate emits it
    // once per run with the full resolved config.
    let mut config_obj: Option<serde_json::Value> = None;
    for line in log.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if entry.get("msg").and_then(|v| v.as_str()) == Some(PACKAGE_FILES_MSG) {
            config_obj = entry.get("packageFiles").cloned();
        }
    }

    let config = config_obj
        .ok_or_else(|| anyhow::anyhow!("'{PACKAGE_FILES_MSG}' not found in Renovate log"))?;

    let mut deps_by_file: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

    if let Some(obj) = config.as_object() {
        for (manager, manager_files) in obj {
            if exclude.contains(manager.as_str()) {
                continue;
            }
            let Some(files) = manager_files.as_array() else {
                continue;
            };
            for pkg_file in files {
                let file_path = pkg_file
                    .get("packageFile")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let Some(deps) = pkg_file.get("deps").and_then(|v| v.as_array()) else {
                    continue;
                };
                for dep in deps {
                    let skip_reason = dep.get("skipReason").and_then(|v| v.as_str());
                    if SKIP_REASONS.contains(&skip_reason.unwrap_or("")) {
                        continue;
                    }
                    let Some(dep_name) = dep.get("depName").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    deps_by_file
                        .entry(file_path.clone())
                        .or_default()
                        .entry(manager.clone())
                        .or_default()
                        .insert(dep_name.to_string());
                }
            }
        }
    }

    // BTreeMap + BTreeSet already sorted; convert sets to vecs.
    Ok(deps_by_file
        .into_iter()
        .map(|(file, managers)| {
            let managers = managers
                .into_iter()
                .map(|(m, deps)| (m, deps.into_iter().collect::<Vec<_>>()))
                .collect();
            (file, managers)
        })
        .collect())
}

fn write_snapshot(path: &Path, deps: &DepMap) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(deps)?;
    std::fs::write(path, json + "\n")?;
    Ok(())
}

fn unified_diff(old: &DepMap, new: &DepMap, committed_display: &str) -> String {
    let old_text = serde_json::to_string_pretty(old).unwrap_or_default() + "\n";
    let new_text = serde_json::to_string_pretty(new).unwrap_or_default() + "\n";

    let diff = similar::TextDiff::from_lines(&old_text, &new_text);
    diff.unified_diff()
        .header(committed_display, "generated")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(config_json: &str) -> Vec<u8> {
        format!(r#"{{"msg":"Extracted dependencies","packageFiles":{config_json}}}"#).into_bytes()
    }

    #[allow(clippy::type_complexity)]
    fn dep_map(entries: &[(&str, &[(&str, &[&str])])]) -> DepMap {
        entries
            .iter()
            .map(|(file, managers)| {
                let m = managers
                    .iter()
                    .map(|(mgr, deps)| {
                        (
                            mgr.to_string(),
                            deps.iter().map(|d| d.to_string()).collect(),
                        )
                    })
                    .collect();
                (file.to_string(), m)
            })
            .collect()
    }

    #[test]
    fn extracts_deps_basic() {
        let log = log(
            r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"express"},{"depName":"lodash"}]}]}"#,
        );
        let result = extract_deps(&log, &[]).unwrap();
        assert_eq!(
            result,
            dep_map(&[("package.json", &[("npm", &["express", "lodash"])])])
        );
    }

    #[test]
    fn deps_are_sorted() {
        let log = log(
            r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"zebra"},{"depName":"alpha"},{"depName":"moose"}]}]}"#,
        );
        let result = extract_deps(&log, &[]).unwrap();
        assert_eq!(
            result["package.json"]["npm"],
            vec!["alpha", "moose", "zebra"]
        );
    }

    #[test]
    fn filters_skip_reasons() {
        let log = log(
            r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"keep"},{"depName":"bad1","skipReason":"contains-variable"},{"depName":"bad2","skipReason":"invalid-value"},{"depName":"bad3","skipReason":"invalid-version"}]}]}"#,
        );
        let result = extract_deps(&log, &[]).unwrap();
        assert_eq!(result["package.json"]["npm"], vec!["keep"]);
    }

    #[test]
    fn other_skip_reasons_are_kept() {
        let log = log(
            r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"pinned","skipReason":"pinned-major-version"}]}]}"#,
        );
        let result = extract_deps(&log, &[]).unwrap();
        assert_eq!(result["package.json"]["npm"], vec!["pinned"]);
    }

    #[test]
    fn excludes_managers() {
        let log = log(
            r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"express"}]}],"cargo":[{"packageFile":"Cargo.toml","deps":[{"depName":"tokio"}]}]}"#,
        );
        let result = extract_deps(&log, &["npm".to_string()]).unwrap();
        assert!(!result.contains_key("package.json"));
        assert_eq!(result["Cargo.toml"]["cargo"], vec!["tokio"]);
    }

    #[test]
    fn skips_deps_without_dep_name() {
        let log = log(
            r#"{"npm":[{"packageFile":"package.json","deps":[{"version":"1.0.0"},{"depName":"valid"}]}]}"#,
        );
        let result = extract_deps(&log, &[]).unwrap();
        assert_eq!(result["package.json"]["npm"], vec!["valid"]);
    }

    #[test]
    fn last_package_files_message_wins() {
        let bytes = format!(
            "{}\n{}\n",
            r#"{"msg":"Extracted dependencies","packageFiles":{"npm":[{"packageFile":"a.json","deps":[{"depName":"old"}]}]}}"#,
            r#"{"msg":"Extracted dependencies","packageFiles":{"npm":[{"packageFile":"b.json","deps":[{"depName":"new"}]}]}}"#,
        )
        .into_bytes();
        let result = extract_deps(&bytes, &[]).unwrap();
        assert!(!result.contains_key("a.json"), "should use last entry");
        assert!(result.contains_key("b.json"));
    }

    #[test]
    fn non_json_lines_are_skipped() {
        let bytes =
            b"not json\n{\"msg\":\"Extracted dependencies\",\"packageFiles\":{\"npm\":[{\"packageFile\":\"p.json\",\"deps\":[{\"depName\":\"x\"}]}]}}\nmore garbage\n";
        let result = extract_deps(bytes, &[]).unwrap();
        assert!(result.contains_key("p.json"));
    }

    #[test]
    fn missing_message_returns_error() {
        let bytes = b"{\"msg\":\"something else\"}\n";
        let err = extract_deps(bytes, &[]).unwrap_err();
        assert!(err.to_string().contains(PACKAGE_FILES_MSG));
    }

    #[test]
    fn write_and_read_snapshot_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.json");
        let deps = dep_map(&[
            ("Cargo.toml", &[("cargo", &["serde", "tokio"])]),
            ("package.json", &[("npm", &["express", "lodash"])]),
        ]);
        write_snapshot(&path, &deps).unwrap();
        let read_back: DepMap =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(deps, read_back);
    }

    #[test]
    fn write_snapshot_ends_with_newline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.json");
        write_snapshot(&path, &dep_map(&[])).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.ends_with('\n'));
    }

    #[test]
    fn unified_diff_contains_added_and_removed_lines() {
        let old = dep_map(&[("a.json", &[("npm", &["old-dep"])])]);
        let new = dep_map(&[("a.json", &[("npm", &["new-dep"])])]);
        let diff = unified_diff(&old, &new, ".github/renovate-tracked-deps.json");
        assert!(diff.contains("-"), "should have removals");
        assert!(diff.contains("+"), "should have additions");
        assert!(diff.contains("old-dep"));
        assert!(diff.contains("new-dep"));
    }

    #[test]
    fn unified_diff_header_uses_display_path() {
        let old = dep_map(&[("a.json", &[("npm", &["x"])])]);
        let new = dep_map(&[("a.json", &[("npm", &["y"])])]);
        let diff = unified_diff(&old, &new, "renovate-tracked-deps.json");
        assert!(diff.contains("renovate-tracked-deps.json"));
    }

    #[test]
    fn resolves_supported_renovate_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".renovaterc.json");
        std::fs::write(&config_path, "{}\n").unwrap();

        let resolved = resolve_renovate_config_path(dir.path()).unwrap();

        assert_eq!(resolved, config_path);
    }

    #[test]
    fn missing_supported_renovate_config_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();

        let err = resolve_renovate_config_path(dir.path()).unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("no supported Renovate config file found"));
        assert!(
            RENOVATE_CONFIG_PATTERNS
                .iter()
                .all(|path| msg.contains(path))
        );
    }

    #[test]
    fn committed_path_uses_same_dir_as_found_config() {
        assert_eq!(
            committed_path_for_config(Path::new("renovate.json5")),
            PathBuf::from("renovate-tracked-deps.json")
        );
        assert_eq!(
            committed_path_for_config(Path::new(".github/renovate.json5")),
            PathBuf::from(".github/renovate-tracked-deps.json")
        );
    }

    fn file_list(paths: &[&str], full: bool) -> FileList {
        FileList {
            files: paths.iter().map(PathBuf::from).collect(),
            changed_paths: paths.iter().map(|path| path.to_string()).collect(),
            merge_base: Some("base".to_string()),
            full,
        }
    }

    #[test]
    fn relevant_when_full_mode() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_relevant(&file_list(&[], true), dir.path()));
    }

    #[test]
    fn relevant_when_renovate_config_changed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_relevant(
            &file_list(
                &[dir.path().join(".github/renovate.json5").to_str().unwrap()],
                false
            ),
            dir.path()
        ));
    }

    #[test]
    fn relevant_when_snapshot_changed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        std::fs::write(
            dir.path().join(".github/renovate-tracked-deps.json"),
            "{}\n",
        )
        .unwrap();

        assert!(is_relevant(
            &file_list(
                &[dir
                    .path()
                    .join(".github/renovate-tracked-deps.json")
                    .to_str()
                    .unwrap()],
                false
            ),
            dir.path()
        ));
    }

    #[test]
    fn relevant_when_tracked_manifest_changed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        write_snapshot(
            &dir.path().join(".github/renovate-tracked-deps.json"),
            &dep_map(&[("package.json", &[("npm", &["express"])])]),
        )
        .unwrap();

        assert!(is_relevant(
            &file_list(&[dir.path().join("package.json").to_str().unwrap()], false),
            dir.path()
        ));
    }

    #[test]
    fn not_relevant_for_untracked_change() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        write_snapshot(
            &dir.path().join(".github/renovate-tracked-deps.json"),
            &dep_map(&[("package.json", &[("npm", &["express"])])]),
        )
        .unwrap();

        assert!(!is_relevant(
            &file_list(&[dir.path().join("README.md").to_str().unwrap()], false),
            dir.path()
        ));
    }

    #[test]
    fn relevant_when_snapshot_is_unparseable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        std::fs::write(
            dir.path().join(".github/renovate-tracked-deps.json"),
            "{not json}\n",
        )
        .unwrap();

        assert!(is_relevant(
            &file_list(&[dir.path().join("README.md").to_str().unwrap()], false),
            dir.path()
        ));
    }
}
