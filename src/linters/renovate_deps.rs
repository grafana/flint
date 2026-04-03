use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::config::RenovateDepsConfig;

const COMMITTED_PATH: &str = ".github/renovate-tracked-deps.json";
const SKIP_REASONS: &[&str] = &["contains-variable", "invalid-value", "invalid-version"];

/// `{file_path: {manager: [dep_name, ...]}}` — all collections sorted.
type DepMap = BTreeMap<String, BTreeMap<String, Vec<String>>>;

pub async fn run(
    cfg: &RenovateDepsConfig,
    fix: bool,
    project_root: &Path,
) -> (bool, Vec<u8>, Vec<u8>) {
    let log_bytes = match run_renovate(project_root).await {
        Ok(b) => b,
        Err(e) => {
            return (
                false,
                vec![],
                format!("flint: renovate-deps: {e}\n").into_bytes(),
            );
        }
    };

    let generated = match extract_deps(&log_bytes, &cfg.exclude_managers) {
        Ok(d) => d,
        Err(e) => {
            return (
                false,
                vec![],
                format!("flint: renovate-deps: {e}\n").into_bytes(),
            );
        }
    };

    let committed_path = project_root.join(COMMITTED_PATH);

    if !committed_path.exists() {
        if fix {
            return match write_snapshot(&committed_path, &generated) {
                Ok(()) => (
                    true,
                    b"renovate-tracked-deps.json has been created.\n".to_vec(),
                    vec![],
                ),
                Err(e) => (
                    false,
                    vec![],
                    format!("flint: renovate-deps: {e}\n").into_bytes(),
                ),
            };
        }
        return (
            false,
            vec![],
            format!(
                "ERROR: {COMMITTED_PATH} does not exist.\nRun `flint --fix renovate-deps` to create it.\n"
            )
            .into_bytes(),
        );
    }

    let committed: DepMap = match std::fs::read_to_string(&committed_path)
        .map_err(anyhow::Error::from)
        .and_then(|s| serde_json::from_str(&s).map_err(anyhow::Error::from))
    {
        Ok(d) => d,
        Err(e) => {
            return (
                false,
                vec![],
                format!("flint: renovate-deps: failed to read committed snapshot: {e}\n")
                    .into_bytes(),
            );
        }
    };

    if committed == generated {
        return (
            true,
            b"renovate-tracked-deps.json is up to date.\n".to_vec(),
            vec![],
        );
    }

    let diff = unified_diff(&committed, &generated);

    if fix {
        return match write_snapshot(&committed_path, &generated) {
            Ok(()) => {
                let mut stdout = diff.into_bytes();
                stdout.extend_from_slice(b"renovate-tracked-deps.json has been updated.\n");
                (true, stdout, vec![])
            }
            Err(e) => (
                false,
                vec![],
                format!("flint: renovate-deps: {e}\n").into_bytes(),
            ),
        };
    }

    (
        false,
        diff.into_bytes(),
        b"ERROR: renovate-tracked-deps.json is out of date.\nRun `flint --fix renovate-deps` to update.\n".to_vec(),
    )
}

/// Runs `renovate --platform=local` and returns the combined stdout+stderr log bytes.
async fn run_renovate(project_root: &Path) -> anyhow::Result<Vec<u8>> {
    let config_path = project_root.join(".github").join("renovate.json5");

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
        if entry.get("msg").and_then(|v| v.as_str()) == Some("packageFiles with updates") {
            config_obj = entry.get("config").cloned();
        }
    }

    let config = config_obj
        .ok_or_else(|| anyhow::anyhow!("'packageFiles with updates' not found in Renovate log"))?;

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
        .header(COMMITTED_PATH, "generated")
        .to_string()
}
