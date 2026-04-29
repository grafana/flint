use anyhow::Context;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::config::RenovateDepsConfig;
use crate::files::FileList;
use crate::linters::LinterOutput;
use crate::linters::env;
use crate::registry::{
    AdaptiveRelevanceContext, InitHookContext, PreparedSpecialCheck, SpecialPrepareContext,
    SpecialRunContext, SpecialRunFuture, StaticLinter, StaticSpecialLinter,
};

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
const PACKAGE_FILES_MSGS: &[&str] = &["Extracted dependencies", "packageFiles with updates"];
const RENOVATE_GITHUB_TOKEN_DISPLAY: &str = "GITHUB_COM_TOKEN or GITHUB_TOKEN";
const SKIP_REASONS: &[&str] = &["contains-variable", "invalid-value", "invalid-version"];

pub(crate) static LINTER: StaticLinter = StaticLinter::special_with_init_hook(
    "renovate-deps",
    StaticSpecialLinter::with_bin("renovate", true, prepare),
    init,
);

#[derive(Debug)]
struct PreparedRenovateDeps {
    name: String,
    cfg: RenovateDepsConfig,
    tracked_files: Vec<PathBuf>,
}

fn prepare(ctx: SpecialPrepareContext<'_>) -> Option<Box<dyn PreparedSpecialCheck>> {
    Some(Box::new(PreparedRenovateDeps {
        name: ctx.name.to_string(),
        cfg: ctx.cfg.checks.renovate_deps.clone(),
        tracked_files: COMMITTED_PATHS
            .iter()
            .map(|path| ctx.project_root.join(path))
            .collect(),
    }))
}

impl PreparedSpecialCheck for PreparedRenovateDeps {
    fn name(&self) -> &str {
        &self.name
    }

    fn tracked_files(&self) -> &[PathBuf] {
        &self.tracked_files
    }

    fn run(self: Box<Self>, ctx: SpecialRunContext) -> SpecialRunFuture {
        Box::pin(async move {
            crate::linters::renovate_deps::run(&self.cfg, ctx.fix, &ctx.project_root).await
        })
    }
}

/// `{file_path: {manager: [dep_name, ...]}}` — all collections sorted.
type DepMap = BTreeMap<String, BTreeMap<String, Vec<String>>>;

pub async fn run(cfg: &RenovateDepsConfig, fix: bool, project_root: &Path) -> LinterOutput {
    match validate_runtime_env() {
        Ok(Some(warning)) => eprintln!("{warning}"),
        Ok(None) => {}
        Err(stderr) => return LinterOutput::err(stderr),
    }
    match run_inner(cfg, fix, project_root).await {
        Ok(out) => out,
        Err(e) => LinterOutput::err(format!("flint: renovate-deps: {e}\n")),
    }
}

pub(crate) fn init(ctx: &dyn InitHookContext) -> anyhow::Result<bool> {
    let toml_path = ctx.config_dir().join("flint.toml");
    let config_changed = if let Some(managers) = ctx.renovate_exclude_managers()
        && !managers.is_empty()
    {
        configure_renovate_deps_config(&toml_path, Some(managers))?
    } else if ctx.flint_toml_generated() {
        configure_renovate_deps_config(&toml_path, None)?
    } else {
        false
    };
    let preset_changed = patch_renovate_preset(ctx.project_root())?;
    Ok(config_changed || preset_changed)
}

fn validate_runtime_env() -> Result<Option<String>, String> {
    validate_runtime_env_from(|name| std::env::var(name).ok())
}

fn validate_runtime_env_from<F>(env: F) -> Result<Option<String>, String>
where
    F: Fn(&str) -> Option<String>,
{
    if env::renovate_github_token_available(&env) {
        return Ok(None);
    }
    if env::is_ci_from(&env) {
        return Err(format!(
            "flint: renovate-deps: missing required CI environment variable: {token_display}\n  Set {github_token}, or set {github_com_token} directly, so Renovate can authenticate GitHub requests in CI.\n",
            token_display = RENOVATE_GITHUB_TOKEN_DISPLAY,
            github_com_token = env::GITHUB_COM_TOKEN_ENV,
            github_token = env::GITHUB_TOKEN_ENV,
        ));
    }
    Ok(Some(env::token_warning(
        "renovate-deps",
        RENOVATE_GITHUB_TOKEN_DISPLAY,
    )))
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

pub(crate) fn adaptive_relevance(ctx: &dyn AdaptiveRelevanceContext) -> bool {
    is_relevant(ctx.file_list(), ctx.project_root())
}

/// Ensures `flint.toml` has the Renovate check config requested by init.
/// Returns `true` when the file was changed.
fn configure_renovate_deps_config(
    toml_path: &Path,
    exclude_managers: Option<&[String]>,
) -> anyhow::Result<bool> {
    let content = std::fs::read_to_string(toml_path)
        .with_context(|| format!("failed to read {}", toml_path.display()))?;
    let mut doc: toml_edit::DocumentMut = content.parse().context("failed to parse flint.toml")?;
    let Some(checks) = doc.get("checks").and_then(|item| item.as_table()) else {
        return append_renovate_deps_config(toml_path, &content, exclude_managers);
    };
    let Some(table_key) = ["renovate-deps", "renovate_deps"]
        .into_iter()
        .find(|key| checks.contains_key(key))
    else {
        return append_renovate_deps_config(toml_path, &content, exclude_managers);
    };

    let Some(managers) = exclude_managers.filter(|managers| !managers.is_empty()) else {
        return Ok(false);
    };
    let renovate = doc
        .get_mut("checks")
        .and_then(|item| item.as_table_mut())
        .and_then(|checks| checks.get_mut(table_key))
        .and_then(|item| item.as_table_mut())
        .with_context(|| {
            format!(
                "[checks.{table_key}] is not a table in {}",
                toml_path.display()
            )
        })?;
    if renovate.contains_key("exclude_managers") {
        return Ok(false);
    }
    renovate.insert("exclude_managers", toml_edit::value(string_array(managers)));
    std::fs::write(toml_path, doc.to_string())
        .with_context(|| format!("failed to write {}", toml_path.display()))?;
    println!(
        "  patched {} — added checks.renovate-deps.exclude_managers",
        toml_path.display()
    );
    Ok(true)
}

fn append_renovate_deps_config(
    toml_path: &Path,
    content: &str,
    exclude_managers: Option<&[String]>,
) -> anyhow::Result<bool> {
    let mut next = String::from(content);
    if !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("\n[checks.renovate-deps]\n");
    match exclude_managers {
        Some(managers) if !managers.is_empty() => {
            next.push_str(&format!("exclude_managers = {}\n", string_array(managers)));
        }
        _ => next.push_str("# exclude_managers = []\n"),
    }
    std::fs::write(toml_path, next)
        .with_context(|| format!("failed to write {}", toml_path.display()))?;
    println!(
        "  patched {} — added checks.renovate-deps",
        toml_path.display()
    );
    Ok(true)
}

fn string_array(values: &[String]) -> toml_edit::Array {
    let mut array = toml_edit::Array::default();
    for value in values {
        array.push(value.as_str());
    }
    array
}

fn patch_renovate_preset(project_root: &Path) -> anyhow::Result<bool> {
    let Some(path) = find_renovate_config(project_root) else {
        return Ok(false);
    };
    let changed = patch_renovate_extends(&path)?;
    if changed {
        let rel = path.strip_prefix(project_root).unwrap_or(&path);
        println!("  patched {} — added {}", rel.display(), flint_preset());
    }
    Ok(changed)
}

fn find_renovate_config(project_root: &Path) -> Option<PathBuf> {
    RENOVATE_CONFIG_PATTERNS
        .iter()
        .map(|path| project_root.join(path))
        .find(|path| path.exists())
}

/// Returns the renovate preset entry to inject, e.g. `github>grafana/flint#v0.9.2`.
/// Pre-release suffixes are stripped so dev builds produce a valid tag reference.
fn flint_preset() -> String {
    let ver = env!("CARGO_PKG_VERSION");
    let ver = ver.split('-').next().unwrap_or(ver);
    format!("github>grafana/flint#v{ver}")
}

/// Adds the flint renovate preset to the `extends` array in a renovate config file.
/// Works for both JSON and JSON5. If an unpinned or differently-pinned flint entry
/// already exists, it is replaced in-place rather than duplicated.
/// Returns `true` if the file was changed.
fn patch_renovate_extends(path: &Path) -> anyhow::Result<bool> {
    let entry = flint_preset();
    let content = std::fs::read_to_string(path)?;

    if content.contains(&entry) {
        return Ok(false);
    }

    // If an existing flint entry (any pin) is present, replace it in-place.
    const FLINT_ENTRY_PREFIX: &str = "\"github>grafana/flint";
    let new_content = if let Some(pos) = content.find(FLINT_ENTRY_PREFIX) {
        let after_open = pos + 1; // skip leading "
        let close = content[after_open..]
            .find('"')
            .context("unclosed quote in existing flint preset entry")?;
        let end = after_open + close + 1; // position after closing "
        format!("{}\"{}\"{}", &content[..pos], entry, &content[end..])
    } else {
        add_to_extends(&content, &entry)
            .with_context(|| format!("failed to patch extends in {}", path.display()))?
    };

    std::fs::write(path, new_content)?;
    Ok(true)
}

/// Text-based insertion of `entry` into the `extends` array.
/// Works for both JSON (`"extends": [`) and JSON5 (`extends: [`).
fn add_to_extends(content: &str, entry: &str) -> anyhow::Result<String> {
    let re = regex::Regex::new(r#"(?:"extends"|extends)\s*:\s*\["#).unwrap();

    if let Some(m) = re.find(content) {
        let bracket_pos = m.end() - 1; // index of '['
        let inside_start = bracket_pos + 1;

        let close_offset = content[inside_start..]
            .find(']')
            .context("extends array has no closing ]")?;
        let close_pos = inside_start + close_offset;
        let inside = &content[inside_start..close_pos];

        if inside.contains('\n') {
            // Multiline: detect indent from first non-empty line, insert at top
            let indent = inside
                .lines()
                .find(|line| !line.trim().is_empty())
                .map(|line| " ".repeat(line.len() - line.trim_start().len()))
                .unwrap_or_else(|| "  ".to_string());
            Ok(format!(
                "{}\n{}\"{}\"{}{}",
                &content[..inside_start],
                indent,
                entry,
                ",",
                &content[inside_start..]
            ))
        } else {
            // Single-line (empty or not): prepend entry
            let sep = if inside.trim().is_empty() { "" } else { ", " };
            Ok(format!(
                "{}\"{}\"{}{}",
                &content[..inside_start],
                entry,
                sep,
                &content[inside_start..]
            ))
        }
    } else {
        // No extends key — add after the opening {
        let open = content
            .find('{')
            .context("no opening { in renovate config")?;
        let (before, after) = content.split_at(open + 1);
        Ok(format!(
            "{}\n  \"extends\": [\"{}\"],{}",
            before, entry, after
        ))
    }
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
    let has_com_token = std::env::var(env::GITHUB_COM_TOKEN_ENV)
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if !has_com_token
        && let Ok(token) = std::env::var(env::GITHUB_TOKEN_ENV)
        && !token.is_empty()
    {
        env.push((env::GITHUB_COM_TOKEN_ENV.into(), token));
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
        if entry
            .get("msg")
            .and_then(|v| v.as_str())
            .is_some_and(|msg| PACKAGE_FILES_MSGS.contains(&msg))
        {
            let extracted_config = entry
                .get("packageFiles")
                .cloned()
                .or_else(|| entry.get("config").cloned());
            if extracted_config.is_some() {
                config_obj = extracted_config;
            }
        }
    }

    let config = config_obj
        .ok_or_else(|| anyhow::anyhow!("none of {:?} found in Renovate log", PACKAGE_FILES_MSGS))?;

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

    fn log_current(config_json: &str) -> Vec<u8> {
        format!(r#"{{"msg":"packageFiles with updates","config":{config_json}}}"#).into_bytes()
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

    fn validate_env(vars: &[(&str, &str)]) -> Result<Option<String>, String> {
        let vars: std::collections::HashMap<String, String> = vars
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        validate_runtime_env_from(|name| vars.get(name).cloned())
    }

    fn write_tmp(content: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), content).unwrap();
        file
    }

    #[test]
    fn configure_renovate_deps_appends_placeholder() {
        let tmp = write_tmp("[settings]\n");
        let changed = configure_renovate_deps_config(tmp.path(), None).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(result.contains("[checks.renovate-deps]"));
        assert!(result.contains("# exclude_managers = []"));
    }

    #[test]
    fn configure_renovate_deps_appends_migrated_managers() {
        let tmp = write_tmp("[settings]\n");
        let managers = vec!["github-actions".to_string(), "cargo".to_string()];
        let changed = configure_renovate_deps_config(tmp.path(), Some(&managers)).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(
            result.contains("exclude_managers = [\"github-actions\", \"cargo\"]"),
            "managers written uncommented: {result}"
        );
        assert!(!result.contains("# exclude_managers"));
    }

    #[test]
    fn configure_renovate_deps_keeps_existing_managers() {
        let tmp = write_tmp("[checks.renovate-deps]\nexclude_managers = [\"npm\"]\n");
        let managers = vec!["github-actions".to_string(), "cargo".to_string()];
        let changed = configure_renovate_deps_config(tmp.path(), Some(&managers)).unwrap();
        assert!(!changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(result.contains("exclude_managers = [\"npm\"]"));
        assert!(!result.contains("github-actions"));
    }

    #[test]
    fn replaces_unpinned_flint_entry_in_place() {
        let input = r#"{ extends: ["config:recommended", "github>grafana/flint"] }"#;
        let tmp = write_tmp(input);
        let changed = patch_renovate_extends(tmp.path()).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(
            result.contains("github>grafana/flint#v"),
            "pinned entry written: {result}"
        );
        assert_eq!(
            result.matches("grafana/flint").count(),
            1,
            "no duplicate: {result}"
        );
        assert!(
            !result.contains("\"github>grafana/flint\""),
            "unpinned removed: {result}"
        );
    }

    #[test]
    fn replaces_differently_pinned_flint_entry() {
        let input = r#"{ extends: ["config:recommended", "github>grafana/flint#v0.5.0"] }"#;
        let tmp = write_tmp(input);
        let changed = patch_renovate_extends(tmp.path()).unwrap();
        assert!(changed);
        let result = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(!result.contains("v0.5.0"), "old pin removed: {result}");
        assert_eq!(
            result.matches("grafana/flint").count(),
            1,
            "no duplicate: {result}"
        );
    }

    #[test]
    fn no_op_when_already_pinned_to_current_version() {
        let entry = flint_preset();
        let input = format!(r#"{{ extends: ["config:recommended", "{entry}"] }}"#);
        let tmp = write_tmp(&input);
        let changed = patch_renovate_extends(tmp.path()).unwrap();
        assert!(!changed);
    }

    #[test]
    fn adds_to_single_line_extends() {
        let input = r#"{ "extends": ["config:recommended"], "other": 1 }"#;
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains(r#"["github>grafana/flint#v0.9.2", "config:recommended"]"#));
    }

    #[test]
    fn adds_to_json5_unquoted_key() {
        let input = "{\n  extends: [\"config:recommended\"],\n}\n";
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains(r#""github>grafana/flint#v0.9.2", "config:recommended""#));
    }

    #[test]
    fn adds_to_multiline_extends() {
        let input = "{\n  extends: [\n    \"config:recommended\",\n    \"other\"\n  ]\n}\n";
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains("\"github>grafana/flint#v0.9.2\","));
        let flint_pos = result.find("grafana/flint").unwrap();
        let existing_pos = result.find("config:recommended").unwrap();
        assert!(flint_pos < existing_pos);
    }

    #[test]
    fn adds_extends_when_absent() {
        let input = "{\n  \"branchPrefix\": \"renovate/\"\n}\n";
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains("\"extends\""));
        assert!(result.contains("github>grafana/flint#v0.9.2"));
    }

    #[test]
    fn adds_to_empty_extends_array() {
        let input = r#"{ "extends": [] }"#;
        let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
        assert!(result.contains(r#"["github>grafana/flint#v0.9.2"]"#));
    }

    #[test]
    fn ci_requires_github_token_or_github_com_token() {
        let err = validate_env(&[("CI", "true")]).unwrap_err();

        assert!(err.contains("GITHUB_COM_TOKEN"), "unexpected error:\n{err}");
        assert!(err.contains("GITHUB_TOKEN"), "unexpected error:\n{err}");
    }

    #[test]
    fn ci_accepts_github_token() {
        let result = validate_env(&[("CI", "true"), ("GITHUB_TOKEN", "token")]);

        assert!(result.is_ok(), "unexpected validation error: {result:?}");
    }

    #[test]
    fn ci_accepts_github_com_token() {
        let result = validate_env(&[("CI", "true"), ("GITHUB_COM_TOKEN", "token")]);

        assert!(result.is_ok(), "unexpected validation error: {result:?}");
    }

    #[test]
    fn non_ci_missing_github_token_warns_without_failing() {
        let warning = validate_env(&[]).unwrap().unwrap();

        assert!(warning.contains("renovate-deps"));
        assert!(warning.contains("GITHUB_TOKEN"));
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
    fn extracts_deps_from_current_renovate_message() {
        let log = log_current(
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
        assert!(err.to_string().contains("none of"));
        assert!(err.to_string().contains(PACKAGE_FILES_MSGS[0]));
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
