use anyhow::Context;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use self::install_patch::configure_extract_workaround_env;
use self::rules::{
    comparable_package_rules_for_config, metadata_lookup_reason, trim_snapshot_meta,
    validate_rule_coverage,
};
use self::snapshot::{Snapshot, extract_deps, read_snapshot, unified_diff, write_snapshot};
use crate::config::RenovateDepsConfig;
use crate::files::FileList;
use crate::linters::LinterOutput;
use crate::linters::env;
use crate::registry::{
    AdaptiveRelevanceContext, CheckTypeDef, InitHookContext, NativeCheckDef, NativePrepareContext,
    NativeRunContext, NativeRunFuture, PreparedNativeCheck,
};

mod install_patch;
mod rules;
mod snapshot;

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
const RENOVATE_GITHUB_TOKEN_DISPLAY: &str = "GITHUB_COM_TOKEN or GITHUB_TOKEN";

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::native_with_init_hook(
    "renovate-deps",
    NativeCheckDef::with_bin("renovate", prepare).with_fix(),
    init,
);

#[derive(Debug)]
struct PreparedRenovateDeps {
    name: String,
    cfg: RenovateDepsConfig,
    config_changed: bool,
    tracked_files: Vec<PathBuf>,
}

fn prepare(ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>> {
    Some(Box::new(PreparedRenovateDeps {
        name: ctx.name.to_string(),
        cfg: ctx.cfg.checks.renovate_deps.clone(),
        config_changed: config_changed(ctx.file_list, ctx.project_root),
        tracked_files: COMMITTED_PATHS
            .iter()
            .map(|path| ctx.project_root.join(path))
            .collect(),
    }))
}

impl PreparedNativeCheck for PreparedRenovateDeps {
    fn name(&self) -> &str {
        &self.name
    }

    fn tracked_files(&self) -> &[PathBuf] {
        &self.tracked_files
    }

    fn run(self: Box<Self>, ctx: NativeRunContext) -> NativeRunFuture {
        Box::pin(async move {
            crate::linters::renovate_deps::run(
                &self.cfg,
                self.config_changed,
                ctx.fix,
                ctx.verbose,
                &ctx.project_root,
            )
            .await
        })
    }
}

pub async fn run(
    cfg: &RenovateDepsConfig,
    config_changed: bool,
    fix: bool,
    verbose: bool,
    project_root: &Path,
) -> LinterOutput {
    match validate_runtime_env() {
        Ok(Some(warning)) => eprintln!("{warning}"),
        Ok(None) => {}
        Err(stderr) => return LinterOutput::err(stderr),
    }
    match run_inner(cfg, config_changed, fix, verbose, project_root).await {
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

    let changed = changed_rel_paths(file_list, project_root);

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

    let committed = match std::fs::read_to_string(&committed_path)
        .ok()
        .and_then(|contents| read_snapshot(&contents).ok())
    {
        Some(committed) => committed,
        None => return true,
    };

    committed.files.keys().any(|path| changed.contains(path))
}

fn changed_rel_paths(file_list: &FileList, project_root: &Path) -> HashSet<String> {
    if !file_list.changed_paths.is_empty() {
        return file_list
            .changed_paths
            .iter()
            .map(|path| {
                let path = Path::new(path);
                path.strip_prefix(project_root).unwrap_or(path)
            })
            .map(normalize_path)
            .collect();
    }

    file_list
        .files
        .iter()
        .filter_map(|path| path.strip_prefix(project_root).ok())
        .map(normalize_path)
        .collect()
}

fn config_changed(file_list: &FileList, project_root: &Path) -> bool {
    if file_list.full {
        return false;
    }
    let changed = changed_rel_paths(file_list, project_root);
    changed
        .iter()
        .any(|path| RENOVATE_CONFIG_PATTERNS.contains(&path.as_str()))
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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
        let separator = if after.trim() == "}" { "" } else { "," };
        Ok(format!(
            "{}\n  \"extends\": [\"{}\"]{}{}",
            before, entry, separator, after
        ))
    }
}

async fn run_inner(
    cfg: &RenovateDepsConfig,
    config_changed: bool,
    fix: bool,
    verbose: bool,
    project_root: &Path,
) -> anyhow::Result<LinterOutput> {
    let config_path = resolve_renovate_config_path(project_root)?;
    let parsed_rules = comparable_package_rules_for_config(&config_path)?;
    let rules = parsed_rules.rules;
    let skipped_rule_notes = parsed_rules.skipped_notes;
    let committed_path = committed_path_for_config(&config_path);
    let committed_display = display_path(project_root, &committed_path);
    let committed = if committed_path.exists() {
        Some(read_snapshot(&std::fs::read_to_string(&committed_path)?)?)
    } else {
        None
    };

    let extracted =
        generate_snapshot(project_root, &config_path, &cfg.exclude_managers, "extract").await?;
    let mut generated = extracted.clone();
    maybe_reuse_committed_meta(&mut generated, committed.as_ref());

    let lookup_reason = metadata_lookup_reason(&generated, &rules);
    if verbose && let Some(reason) = lookup_reason.as_deref() {
        if config_changed || fix {
            eprintln!("flint: renovate-deps: lookup required: {reason}");
        } else {
            eprintln!(
                "flint: renovate-deps: lookup skipped because Renovate config is unchanged: {reason}"
            );
        }
    }

    if !fix && !config_changed && lookup_reason.is_some() {
        anyhow::bail!(
            "dependency metadata is out of date for rule-coverage validation.\nRun `flint run --fix renovate-deps` to refresh metadata."
        );
    }

    if lookup_reason.is_some() && (config_changed || fix) {
        generated =
            generate_snapshot(project_root, &config_path, &cfg.exclude_managers, "lookup").await?;
    }

    validate_rule_coverage(&generated, &rules)?;
    trim_snapshot_meta(&mut generated, &rules);

    if committed.is_none() {
        if fix {
            write_snapshot(&committed_path, &generated)?;
            let mut stdout = notes_output(&skipped_rule_notes).into_bytes();
            stdout.extend_from_slice(format!("{COMMITTED_FILE} has been created.\n").as_bytes());
            return Ok(LinterOutput {
                ok: true,
                stdout,
                stderr: vec![],
                setup_outcome: None,
            });
        }
        return Ok(LinterOutput::err(format!(
            "ERROR: {committed_display} does not exist.\nRun `flint run --fix renovate-deps` to create it.\n"
        )));
    }

    let committed = committed.expect("checked above");

    if committed == generated {
        let mut stdout = notes_output(&skipped_rule_notes).into_bytes();
        stdout.extend_from_slice(format!("{COMMITTED_FILE} is up to date.\n").as_bytes());
        return Ok(LinterOutput {
            ok: true,
            stdout,
            stderr: vec![],
            setup_outcome: None,
        });
    }

    let diff = unified_diff(&committed, &generated, &committed_display);

    if fix {
        write_snapshot(&committed_path, &generated)?;
        let mut stdout = notes_output(&skipped_rule_notes).into_bytes();
        stdout.extend_from_slice(diff.as_bytes());
        stdout.extend_from_slice(format!("{COMMITTED_FILE} has been updated.\n").as_bytes());
        return Ok(LinterOutput {
            ok: true,
            stdout,
            stderr: vec![],
            setup_outcome: None,
        });
    }

    Ok(LinterOutput {
        ok: false,
        stdout: diff.into_bytes(),
        stderr: format!(
            "ERROR: {COMMITTED_FILE} is out of date.\nRun `flint run --fix renovate-deps` to update.\n"
        )
        .into_bytes(),
        setup_outcome: None,
    })
}

fn notes_output(notes: &[String]) -> String {
    if notes.is_empty() {
        return String::new();
    }

    format!("{}\n", notes.join("\n"))
}

async fn generate_snapshot(
    project_root: &Path,
    config_path: &Path,
    exclude_managers: &[String],
    dry_run: &str,
) -> anyhow::Result<Snapshot> {
    // Renovate occasionally produces empty packageFiles on the first run (transient
    // package cache, registry, or startup issue). Retry up to 3 times with a short delay.
    let mut generated = Snapshot::default();
    for attempt in 1..=3u32 {
        let log_bytes = run_renovate(project_root, config_path, dry_run).await?;
        generated = extract_deps(&log_bytes, exclude_managers)?;
        if !generated.is_empty() || attempt == 3 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
    Ok(generated)
}

/// Runs `renovate --platform=local` and returns the combined stdout+stderr log bytes.
async fn run_renovate(
    project_root: &Path,
    config_path: &Path,
    dry_run: &str,
) -> anyhow::Result<Vec<u8>> {
    // Forward env, setting Renovate-specific vars.
    let mut env: Vec<(String, String)> = std::env::vars().collect();
    configure_extract_workaround_env(&mut env, dry_run)?;
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
            format!("--dry-run={dry_run}"),
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
    normalize_path(path.strip_prefix(project_root).unwrap_or(path))
}

fn merge_missing_meta_from_committed(generated: &mut Snapshot, committed: &Snapshot) {
    for (dep_name, generated_meta) in &mut generated.meta {
        let Some(committed_meta) = committed.meta.get(dep_name) else {
            continue;
        };
        if generated_meta.package_name.is_none() {
            generated_meta.package_name = committed_meta.package_name.clone();
        }
        if generated_meta.datasource.is_none() {
            generated_meta.datasource = committed_meta.datasource.clone();
        }
    }
}

fn maybe_reuse_committed_meta(generated: &mut Snapshot, committed: Option<&Snapshot>) {
    if let Some(committed) = committed {
        merge_missing_meta_from_committed(generated, committed);
    }
}

#[cfg(test)]
mod tests;
