use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::config::RenovateDepsConfig;
use crate::linters::LinterOutput;

const COMMITTED_DIR: &str = ".github";
const COMMITTED_FILE: &str = "renovate-tracked-deps.json";
const COMMITTED_DISPLAY: &str = ".github/renovate-tracked-deps.json";
const RENOVATE_CONFIG_FILE: &str = "renovate.json5";
const PACKAGE_FILES_MSG: &str = "packageFiles with updates";
const SKIP_REASONS: &[&str] = &["contains-variable", "invalid-value", "invalid-version"];

/// `{file_path: {manager: [dep_name, ...]}}` — all collections sorted.
type DepMap = BTreeMap<String, BTreeMap<String, Vec<String>>>;

pub async fn run(cfg: &RenovateDepsConfig, fix: bool, project_root: &Path) -> LinterOutput {
    match run_inner(cfg, fix, project_root).await {
        Ok(out) => out,
        Err(e) => LinterOutput::err(format!("flint: renovate-deps: {e}\n")),
    }
}

async fn run_inner(
    cfg: &RenovateDepsConfig,
    fix: bool,
    project_root: &Path,
) -> anyhow::Result<LinterOutput> {
    let log_bytes = run_renovate(project_root).await?;
    let generated = extract_deps(&log_bytes, &cfg.exclude_managers)?;
    let committed_path = project_root.join(COMMITTED_DIR).join(COMMITTED_FILE);

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
            "ERROR: {COMMITTED_DISPLAY} does not exist.\nRun `flint --fix renovate-deps` to create it.\n"
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

    let diff = unified_diff(&committed, &generated);

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
            "ERROR: {COMMITTED_FILE} is out of date.\nRun `flint --fix renovate-deps` to update.\n"
        )
        .into_bytes(),
    })
}

/// Runs `renovate --platform=local` and returns the combined stdout+stderr log bytes.
async fn run_renovate(project_root: &Path) -> anyhow::Result<Vec<u8>> {
    let config_path = project_root.join(COMMITTED_DIR).join(RENOVATE_CONFIG_FILE);

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

    let out = Command::new("renovate")
        .args(["--platform=local", "--require-config=ignored"])
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
            config_obj = entry.get("config").cloned();
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

fn unified_diff(old: &DepMap, new: &DepMap) -> String {
    let old_text = serde_json::to_string_pretty(old).unwrap_or_default() + "\n";
    let new_text = serde_json::to_string_pretty(new).unwrap_or_default() + "\n";

    let diff = similar::TextDiff::from_lines(&old_text, &new_text);
    diff.unified_diff()
        .header(COMMITTED_DISPLAY, "generated")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(config_json: &str) -> Vec<u8> {
        format!(r#"{{"msg":"packageFiles with updates","config":{config_json}}}"#).into_bytes()
    }

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
            r#"{"msg":"packageFiles with updates","config":{"npm":[{"packageFile":"a.json","deps":[{"depName":"old"}]}]}}"#,
            r#"{"msg":"packageFiles with updates","config":{"npm":[{"packageFile":"b.json","deps":[{"depName":"new"}]}]}}"#,
        )
        .into_bytes();
        let result = extract_deps(&bytes, &[]).unwrap();
        assert!(!result.contains_key("a.json"), "should use last entry");
        assert!(result.contains_key("b.json"));
    }

    #[test]
    fn non_json_lines_are_skipped() {
        let bytes =
            b"not json\n{\"msg\":\"packageFiles with updates\",\"config\":{\"npm\":[{\"packageFile\":\"p.json\",\"deps\":[{\"depName\":\"x\"}]}]}}\nmore garbage\n";
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
        let diff = unified_diff(&old, &new);
        assert!(diff.contains("-"), "should have removals");
        assert!(diff.contains("+"), "should have additions");
        assert!(diff.contains("old-dep"));
        assert!(diff.contains("new-dep"));
    }

    #[test]
    fn unified_diff_header_uses_display_path() {
        let old = dep_map(&[("a.json", &[("npm", &["x"])])]);
        let new = dep_map(&[("a.json", &[("npm", &["y"])])]);
        let diff = unified_diff(&old, &new);
        assert!(diff.contains(COMMITTED_DISPLAY));
    }
}
