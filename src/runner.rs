use anyhow::Result;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

use crate::config::{Config, LicenseHeaderConfig, LycheeConfig, RenovateDepsConfig, Settings};
use crate::files::FileList;
use crate::linters::{LinterOutput, flint_setup, license_header, lychee, renovate_deps};
use crate::registry::{Check, CheckKind, LinterConfig, Scope, SpecialKind};

#[derive(Clone, Copy)]
pub struct RunOptions {
    pub fix: bool,
    pub verbose: bool,
    pub short: bool,
    pub time: bool,
}

pub struct CheckResult {
    pub name: String,
    pub ok: bool,
    pub changed: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration: Duration,
}

#[derive(Clone, Copy)]
struct InvocationOutputPolicy<'a> {
    nonverbose: bool,
    env: &'a [(&'static str, &'static str)],
    nonverbose_filter_prefixes: &'a [&'static str],
    stderr_filter_prefixes: &'a [&'static str],
}

/// A check with all inputs pre-resolved, ready to execute without borrowing
/// the registry or config. Built by `prepare()` before the fix/check split.
enum PreparedCheck {
    Invocations {
        name: String,
        argv_list: Vec<Vec<String>>,
        tracked_files: Vec<PathBuf>,
        windows_java_jar: bool,
        env: &'static [(&'static str, &'static str)],
        nonverbose_filter_prefixes: &'static [&'static str],
        stderr_filter_prefixes: &'static [&'static str],
    },
    Links {
        name: String,
        cfg: LycheeConfig,
        settings: Settings,
        file_list: FileList,
        config_dir: PathBuf,
    },
    RenovateDeps {
        name: String,
        cfg: RenovateDepsConfig,
        tracked_files: Vec<PathBuf>,
    },
    LicenseHeader {
        name: String,
        cfg: LicenseHeaderConfig,
        files: Vec<PathBuf>,
    },
    FlintSetup {
        name: String,
        path: PathBuf,
        config_dir: PathBuf,
        setup_migration_version: u32,
    },
}

impl PreparedCheck {
    fn name(&self) -> &str {
        match self {
            Self::Invocations { name, .. }
            | Self::Links { name, .. }
            | Self::RenovateDeps { name, .. }
            | Self::LicenseHeader { name, .. }
            | Self::FlintSetup { name, .. } => name,
        }
    }

    async fn execute(self, fix: bool, verbose: bool, project_root: &Path) -> CheckResult {
        let name = self.name().to_string();
        let start = Instant::now();
        let (out, changed): (LinterOutput, bool) = match self {
            Self::Invocations {
                argv_list,
                tracked_files,
                windows_java_jar,
                env,
                nonverbose_filter_prefixes,
                stderr_filter_prefixes,
                ..
            } => {
                let before = if fix && !tracked_files.is_empty() {
                    Some(fingerprint_files(&tracked_files))
                } else {
                    None
                };
                let out = run_invocations(
                    &name,
                    &argv_list,
                    windows_java_jar,
                    InvocationOutputPolicy {
                        nonverbose: !verbose,
                        env: if verbose { &[] } else { env },
                        nonverbose_filter_prefixes: if verbose {
                            &[]
                        } else {
                            nonverbose_filter_prefixes
                        },
                        stderr_filter_prefixes: if verbose { &[] } else { stderr_filter_prefixes },
                    },
                    project_root,
                )
                .await;
                let changed =
                    before.is_some_and(|before| before != fingerprint_files(&tracked_files));
                (out, changed)
            }
            Self::Links {
                cfg,
                settings,
                file_list,
                config_dir,
                ..
            } => (
                lychee::run(&cfg, &settings, &file_list, project_root, &config_dir).await,
                false,
            ),
            Self::RenovateDeps {
                cfg, tracked_files, ..
            } => {
                let before = if fix && !tracked_files.is_empty() {
                    Some(fingerprint_files(&tracked_files))
                } else {
                    None
                };
                let out = renovate_deps::run(&cfg, fix, project_root).await;
                let changed =
                    before.is_some_and(|before| before != fingerprint_files(&tracked_files));
                (out, changed)
            }
            Self::LicenseHeader { cfg, files, .. } => {
                (license_header::run(&cfg, project_root, &files).await, false)
            }
            Self::FlintSetup {
                path,
                config_dir,
                setup_migration_version,
                ..
            } => {
                let tracked_files = vec![path.clone(), config_dir.join("flint.toml")];
                let before = if fix {
                    Some(fingerprint_files(&tracked_files))
                } else {
                    None
                };
                let out =
                    flint_setup::run(fix, project_root, &config_dir, setup_migration_version).await;
                let changed =
                    before.is_some_and(|before| before != fingerprint_files(&tracked_files));
                (out, changed)
            }
        };
        CheckResult {
            name,
            ok: out.ok,
            changed,
            stdout: out.stdout,
            stderr: out.stderr,
            duration: start.elapsed(),
        }
    }
}

pub async fn run(
    checks: &[&Check],
    active_checks: &[&Check],
    file_list: &FileList,
    opts: RunOptions,
    project_root: &Path,
    cfg: &Config,
    config_dir: &Path,
) -> Result<Vec<CheckResult>> {
    let RunOptions {
        fix,
        verbose,
        short,
        time,
    } = opts;
    let prepared: Vec<PreparedCheck> = checks
        .iter()
        .filter_map(|&check| {
            prepare(
                check,
                file_list,
                fix,
                project_root,
                active_checks,
                cfg,
                config_dir,
            )
        })
        .collect();

    if fix {
        let mut results = vec![];
        for task in prepared {
            let r = task.execute(fix, verbose, project_root).await;
            if !short && (verbose || !r.ok) {
                eprintln!("[{}]{}", r.name, format_duration_suffix(time, r.duration));
                flush_output(&r.stdout, &r.stderr);
            }
            results.push(r);
        }
        return Ok(results);
    }

    let mut set: JoinSet<CheckResult> = JoinSet::new();
    for task in prepared {
        let root = project_root.to_path_buf();
        set.spawn(async move { task.execute(false, verbose, &root).await });
    }

    // Collect all results before printing to avoid interleaved output.
    // Sort by name for deterministic output order.
    let mut collected = vec![];
    while let Some(res) = set.join_next().await {
        collected.push(res?);
    }
    collected.sort_by(|a, b| a.name.cmp(&b.name));

    if !short {
        for r in &collected {
            if verbose || !r.ok || time {
                eprintln!("[{}]{}", r.name, format_duration_suffix(time, r.duration));
            }
            if verbose || !r.ok {
                flush_output(&r.stdout, &r.stderr);
            }
        }
    }

    Ok(collected)
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    check: &Check,
    file_list: &FileList,
    fix: bool,
    project_root: &Path,
    active_checks: &[&Check],
    cfg: &Config,
    config_dir: &Path,
) -> Option<PreparedCheck> {
    let name = check.name.to_string();
    match &check.kind {
        CheckKind::Template { .. } => {
            let tracked_files = tracked_files(check, file_list, project_root, active_checks);
            let argv_list = build_invocations(
                check,
                file_list,
                fix,
                project_root,
                active_checks,
                config_dir,
            );
            if argv_list.is_empty() {
                return None;
            }
            Some(PreparedCheck::Invocations {
                name,
                argv_list,
                tracked_files,
                windows_java_jar: check.windows_java_jar,
                env: check.env,
                nonverbose_filter_prefixes: check.nonverbose_filter_prefixes,
                stderr_filter_prefixes: check.stderr_filter_prefixes,
            })
        }
        CheckKind::Special(special) => match special.kind() {
            SpecialKind::Links => Some(PreparedCheck::Links {
                name,
                cfg: cfg.checks.lychee.clone(),
                settings: cfg.settings.clone(),
                file_list: file_list.clone(),
                config_dir: config_dir.to_path_buf(),
            }),
            SpecialKind::RenovateDeps => Some(PreparedCheck::RenovateDeps {
                name,
                cfg: cfg.checks.renovate_deps.clone(),
                tracked_files: renovate_deps::COMMITTED_PATHS
                    .iter()
                    .map(|path| project_root.join(path))
                    .collect(),
            }),
            SpecialKind::LicenseHeader => {
                if cfg.checks.license_header.text.is_empty() {
                    return None;
                }
                let patterns: Vec<&str> = cfg
                    .checks
                    .license_header
                    .patterns
                    .iter()
                    .map(String::as_str)
                    .collect();
                let files: Vec<PathBuf> =
                    match_files(&file_list.files, &patterns, &[], project_root)
                        .into_iter()
                        .cloned()
                        .collect();
                if files.is_empty() {
                    return None;
                }
                Some(PreparedCheck::LicenseHeader {
                    name,
                    cfg: cfg.checks.license_header.clone(),
                    files,
                })
            }
            SpecialKind::FlintSetup => Some(PreparedCheck::FlintSetup {
                name,
                path: project_root.join("mise.toml"),
                config_dir: config_dir.to_path_buf(),
                setup_migration_version: cfg.settings.setup_migration_version,
            }),
        },
    }
}

fn tracked_files(
    check: &Check,
    file_list: &FileList,
    project_root: &Path,
    active_checks: &[&Check],
) -> Vec<PathBuf> {
    let CheckKind::Template { scope, .. } = &check.kind else {
        return vec![];
    };
    if !matches!(scope, Scope::File | Scope::Files) {
        return vec![];
    }

    let mut excludes: Vec<&str> = active_checks
        .iter()
        .filter(|c| check.excludes_if_active.contains(&c.name))
        .flat_map(|c| c.patterns.iter().copied())
        .collect();
    if check.defers_to_formatters {
        for active in active_checks.iter().filter(|c| c.is_formatter) {
            excludes.extend(active.patterns.iter().copied());
        }
    }

    match_files(&file_list.files, check.patterns, &excludes, project_root)
        .into_iter()
        .cloned()
        .collect()
}

/// Returns the list of argv vectors to execute for a check.
fn build_invocations(
    check: &Check,
    file_list: &FileList,
    fix: bool,
    project_root: &Path,
    active_checks: &[&Check],
    config_dir: &Path,
) -> Vec<Vec<String>> {
    let CheckKind::Template {
        check_cmd,
        fix_cmd,
        full_cmd,
        full_fix_cmd,
        scope,
    } = &check.kind
    else {
        return vec![];
    };

    let cmd_template: &str = if fix && check.has_fix() {
        fix_cmd
    } else {
        check_cmd
    };

    // Collect patterns from checks that are active and listed in excludes_if_active.
    let mut excludes: Vec<&str> = active_checks
        .iter()
        .filter(|c| check.excludes_if_active.contains(&c.name))
        .flat_map(|c| c.patterns.iter().copied())
        .collect();

    // When this check defers to formatters, also exclude files owned by active formatters.
    if check.defers_to_formatters {
        for active in active_checks.iter().filter(|c| c.is_formatter) {
            excludes.extend(active.patterns.iter().copied());
        }
    }

    let config_args = resolve_linter_config(check, config_dir);
    let rendered_config_args = render_config_args(&config_args);
    let inject_config_args = !cmd_template.contains("{CONFIG_ARGS}");

    match scope {
        Scope::Project => {
            // If patterns are set, only run when relevant files are present.
            if !check.patterns.is_empty()
                && match_files(&file_list.files, check.patterns, &excludes, project_root).is_empty()
            {
                return vec![];
            }
            let cmd = substitute_merge_base(cmd_template, file_list.merge_base.as_deref());
            let cmd = cmd.replace("{CONFIG_ARGS}", &rendered_config_args);
            let argv = shell_words(cmd);
            vec![if inject_config_args {
                inject_config(argv, &config_args)
            } else {
                argv
            }]
        }

        Scope::File => {
            let matched = match_files(&file_list.files, check.patterns, &excludes, project_root);
            matched
                .iter()
                .map(|f| {
                    let cmd = cmd_template
                        .replace("{FILE}", &quote_path(f))
                        .replace("{CONFIG_ARGS}", &rendered_config_args);
                    let argv = shell_words(cmd);
                    if inject_config_args {
                        inject_config(argv, &config_args)
                    } else {
                        argv
                    }
                })
                .collect()
        }

        Scope::Files => {
            let matched = match_files(&file_list.files, check.patterns, &excludes, project_root);
            if matched.is_empty() {
                return vec![];
            }
            // When all project files are in scope and a full_cmd is set, use it as a
            // project-wide command instead of passing a (potentially huge) file list.
            if file_list.full {
                let effective = if fix && !full_fix_cmd.is_empty() {
                    Some(*full_fix_cmd)
                } else if !fix && !full_cmd.is_empty() {
                    Some(*full_cmd)
                } else {
                    None
                };
                if let Some(cmd) = effective {
                    let cmd = cmd
                        .replace("{ROOT}", &quote_path(project_root))
                        .replace("{CONFIG_ARGS}", &rendered_config_args);
                    let argv = shell_words(cmd);
                    return vec![if inject_config_args {
                        inject_config(argv, &config_args)
                    } else {
                        argv
                    }];
                }
            }
            let edition_flag = resolve_cargo_edition_flag(project_root);
            let files_arg: String = matched
                .iter()
                .map(|f| quote_path(f))
                .collect::<Vec<_>>()
                .join(" ");
            let rel_files_arg: String = matched
                .iter()
                .map(|f| quote_path(f.strip_prefix(project_root).unwrap_or(f)))
                .collect::<Vec<_>>()
                .join(" ");
            let cmd = cmd_template
                .replace("{CARGO_EDITION_FLAG}", &edition_flag)
                .replace("{FILES}", &files_arg)
                .replace("{RELFILES}", &rel_files_arg)
                .replace("{CONFIG_ARGS}", &rendered_config_args);
            let argv = shell_words(cmd);
            vec![if inject_config_args {
                inject_config(argv, &config_args)
            } else {
                argv
            }]
        }
    }
}

/// Returns `--edition <edition>` if a Rust edition is declared in the project's
/// `Cargo.toml`, or an empty string if not found. Used to substitute
/// `{CARGO_EDITION_FLAG}` in rustfmt command templates.
fn resolve_cargo_edition_flag(project_root: &Path) -> String {
    let Ok(content) = std::fs::read_to_string(project_root.join("Cargo.toml")) else {
        return String::new();
    };
    let Ok(doc) = content.parse::<toml::Value>() else {
        return String::new();
    };
    let edition = doc
        .get("package")
        .and_then(|p| p.get("edition"))
        .and_then(|e| e.as_str())
        .or_else(|| {
            doc.get("workspace")
                .and_then(|w| w.get("package"))
                .and_then(|p| p.get("edition"))
                .and_then(|e| e.as_str())
        });
    edition
        .map(|e| format!("--edition {e}"))
        .unwrap_or_default()
}

/// Returns config args for `check` based on files present in `config_dir`.
fn resolve_linter_config(check: &Check, config_dir: &Path) -> Vec<String> {
    let Some(config) = &check.linter_config else {
        return vec![];
    };
    match config {
        LinterConfig::File { file, flag } => {
            let path = config_dir.join(file);
            if !path.exists() {
                return vec![];
            }
            vec![flag.to_string(), path.to_string_lossy().into_owned()]
        }
        LinterConfig::DirIfAny { files, flag } => {
            if files.iter().any(|file| config_dir.join(file).exists()) {
                vec![flag.to_string(), config_dir.to_string_lossy().into_owned()]
            } else {
                vec![]
            }
        }
    }
}

/// Inserts `config_args` at position 1 (right after the binary name) in `argv`.
fn inject_config(mut argv: Vec<String>, config_args: &[String]) -> Vec<String> {
    if config_args.is_empty() || argv.is_empty() {
        return argv;
    }
    // Insert after argv[0] (the binary name).
    let tail = argv.split_off(1);
    argv.extend_from_slice(config_args);
    argv.extend(tail);
    argv
}

fn render_config_args(config_args: &[String]) -> String {
    config_args
        .iter()
        .map(|arg| quote_path(Path::new(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Runs all invocations for one check.
/// Never prints — callers decide when and whether to flush output.
async fn run_invocations(
    name: &str,
    invocations: &[Vec<String>],
    windows_java_jar: bool,
    output_policy: InvocationOutputPolicy<'_>,
    root: &Path,
) -> LinterOutput {
    let mut all_ok = true;
    let mut combined_stdout = Vec::new();
    let mut combined_stderr = Vec::new();

    for argv in invocations {
        if argv.is_empty() {
            continue;
        }
        let mut cmd = crate::linters::spawn_command(argv, windows_java_jar);
        cmd.current_dir(root)
            .stdin(Stdio::null())
            .envs(output_policy.env.iter().copied());
        let result = cmd.output().await;
        match result {
            Ok(out) => {
                if name == "taplo"
                    && !output_policy.stderr_filter_prefixes.is_empty()
                    && !out.status.success()
                {
                    let (stdout, stderr) =
                        normalize_taplo_nonverbose_output(argv, &out.stdout, &out.stderr);
                    combined_stdout.extend_from_slice(&stdout);
                    combined_stderr.extend_from_slice(&stderr);
                } else {
                    let stdout = if output_policy.nonverbose
                        && !output_policy.nonverbose_filter_prefixes.is_empty()
                    {
                        filter_output_lines(&out.stdout, |line| {
                            output_policy
                                .nonverbose_filter_prefixes
                                .iter()
                                .any(|prefix| line.starts_with(prefix))
                        })
                    } else {
                        out.stdout
                    };
                    combined_stdout.extend_from_slice(&stdout);
                    let stderr = if output_policy.stderr_filter_prefixes.is_empty() {
                        out.stderr
                    } else {
                        filter_stderr_lines(&out.stderr, output_policy.stderr_filter_prefixes)
                    };
                    if output_policy.nonverbose
                        && !output_policy.nonverbose_filter_prefixes.is_empty()
                    {
                        let filtered = filter_output_lines(&stderr, |line| {
                            output_policy
                                .nonverbose_filter_prefixes
                                .iter()
                                .any(|prefix| line.starts_with(prefix))
                        });
                        combined_stderr.extend_from_slice(&filtered);
                    } else {
                        combined_stderr.extend_from_slice(&stderr);
                    }
                }
                if !out.status.success() {
                    all_ok = false;
                }
            }
            Err(e) => {
                combined_stderr
                    .extend_from_slice(format!("flint: {name}: failed to spawn: {e}\n").as_bytes());
                all_ok = false;
            }
        }
    }

    maybe_append_rust_component_note(name, &mut combined_stderr);

    LinterOutput {
        ok: all_ok,
        stdout: combined_stdout,
        stderr: combined_stderr,
    }
}

fn filter_stderr_lines(stderr: &[u8], prefixes: &[&str]) -> Vec<u8> {
    let text = String::from_utf8_lossy(stderr);
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches('\n');
        if prefixes.iter().any(|prefix| trimmed.starts_with(prefix)) {
            continue;
        }
        out.push_str(line);
    }
    if !text.is_empty() && !text.ends_with('\n') {
        let tail = text
            .rsplit_once('\n')
            .map(|(_, tail)| tail)
            .unwrap_or(&text);
        if !prefixes.iter().any(|prefix| tail.starts_with(prefix)) && !out.ends_with(tail) {
            out.push_str(tail);
        }
    }
    out.into_bytes()
}

fn normalize_taplo_nonverbose_output(
    argv: &[String],
    stdout: &[u8],
    stderr: &[u8],
) -> (Vec<u8>, Vec<u8>) {
    let raw = format!(
        "{}{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );
    let mut error_lines: Vec<String> = raw
        .lines()
        .filter(|line| line.starts_with("ERROR"))
        .map(ToOwned::to_owned)
        .collect();

    if error_lines.is_empty()
        && let Some(target) = argv.last()
    {
        error_lines.push(format!(
            "ERROR taplo:format_files: the file is not properly formatted path=\"{target}\""
        ));
    }

    if !error_lines.is_empty()
        && !error_lines.iter().any(|line| {
            line == "ERROR operation failed error=some files were not properly formatted"
        })
    {
        error_lines.push(
            "ERROR operation failed error=some files were not properly formatted".to_string(),
        );
    }

    let stderr = if error_lines.is_empty() {
        Vec::new()
    } else {
        format!("{}\n", error_lines.join("\n")).into_bytes()
    };

    (Vec::new(), stderr)
}

fn filter_output_lines(output: &[u8], predicate: impl Fn(&str) -> bool) -> Vec<u8> {
    let text = String::from_utf8_lossy(output);
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches('\n');
        if predicate(trimmed) {
            continue;
        }
        out.push_str(line);
    }
    if !text.is_empty() && !text.ends_with('\n') {
        let tail = text
            .rsplit_once('\n')
            .map(|(_, tail)| tail)
            .unwrap_or(&text);
        if !predicate(tail) && !out.ends_with(tail) {
            out.push_str(tail);
        }
    }
    out.into_bytes()
}

fn maybe_append_rust_component_note(name: &str, stderr: &mut Vec<u8>) {
    let Some(component) = missing_rust_component(name, stderr) else {
        return;
    };
    let note = format!(
        "NOTE: `{name}` needs the Rust `{component}` component in the active toolchain.\n\
`mise` may activate an existing Rust toolchain without adding missing components.\n\
Install it with: `rustup component add {component}`\n"
    );
    stderr.extend_from_slice(note.as_bytes());
}

fn fingerprint_files(files: &[PathBuf]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for path in files {
        path.hash(&mut hasher);
        if let Ok(bytes) = std::fs::read(path) {
            bytes.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn missing_rust_component(name: &str, stderr: &[u8]) -> Option<&'static str> {
    let stderr = String::from_utf8_lossy(stderr);
    match name {
        "cargo-clippy" if stderr.contains("'cargo-clippy' is not installed for the toolchain") => {
            Some("clippy")
        }
        "cargo-fmt" if stderr.contains("'rustfmt' is not installed for the toolchain") => {
            Some("rustfmt")
        }
        _ => None,
    }
}

fn format_duration_suffix(time: bool, duration: Duration) -> String {
    if !time {
        return String::new();
    }
    let ms = duration.as_millis();
    if ms < 1000 {
        format!(" {ms}ms")
    } else {
        format!(" {:.1}s", duration.as_secs_f64())
    }
}

fn flush_output(stdout: &[u8], stderr: &[u8]) {
    // All tool output goes to stderr so headers and diagnostics stay on the
    // same stream — callers (humans and AI alike) see a coherent sequence.
    if !stdout.is_empty() {
        eprint!("{}", String::from_utf8_lossy(stdout));
    }
    if !stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(stderr));
    }
}

fn match_files<'a>(
    files: &'a [PathBuf],
    patterns: &[&str],
    exclude_patterns: &[&str],
    project_root: &Path,
) -> Vec<&'a PathBuf> {
    files
        .iter()
        .filter(|p| {
            let rel = p.strip_prefix(project_root).unwrap_or(p);
            let rel_str = rel.to_string_lossy();
            let file_name = p
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            let included = patterns.iter().any(|pat| {
                if *pat == "*" {
                    return true;
                }
                glob_match(pat, file_name.as_ref()) || glob_match(pat, rel_str.as_ref())
            });
            let excluded = exclude_patterns.iter().any(|pat| {
                glob_match(pat, file_name.as_ref()) || glob_match(pat, rel_str.as_ref())
            });
            included && !excluded
        })
        .collect()
}

fn glob_match(pattern: &str, name: &str) -> bool {
    // Simple glob: splits on `*` and checks that each segment appears in order.
    // Handles `*.ext`, `prefix*`, `dir/*.yml`, etc.
    let parts: Vec<&str> = pattern.splitn(2, '*').collect();
    match parts.as_slice() {
        [only] => name == *only || name.ends_with(&format!("/{only}")),
        [prefix, suffix] => {
            let n = name;
            // The prefix must match the start of the name (or the part after the last slash).
            let anchor_start = prefix.is_empty() || n.starts_with(prefix) || {
                // Allow matching the basename portion for patterns like `*.sh`.
                n.contains('/') && {
                    let after_slash = n.rfind('/').map(|i| &n[i + 1..]).unwrap_or(n);
                    prefix.is_empty() || after_slash.starts_with(prefix)
                }
            };
            anchor_start && n.ends_with(suffix)
        }
        _ => false,
    }
}

fn substitute_merge_base(cmd: &str, merge_base: Option<&str>) -> String {
    if let Some(base) = merge_base {
        cmd.replace("{MERGE_BASE}", base)
    } else {
        // Strip any flag containing {MERGE_BASE} (e.g. --new-from-rev={MERGE_BASE}).
        cmd.split_whitespace()
            .filter(|tok| !tok.contains("{MERGE_BASE}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn quote_path(p: &Path) -> String {
    let s = p.to_string_lossy();
    format!("\"{}\"", s.replace('"', "\\\""))
}

fn shell_words(cmd: String) -> Vec<String> {
    // Minimal word-splitting that respects single- and double-quoted strings.
    let mut words = vec![];
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\'' if !in_single && !in_double => {
                in_single = true;
            }
            '\'' if in_single => {
                in_single = false;
            }
            '"' if !in_single && !in_double => {
                in_double = true;
            }
            '"' if in_double => {
                in_double = false;
            }
            '\\' if in_double => {
                // Only handle \" inside double quotes; pass other backslashes through.
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    current.push('"');
                    i += 2;
                    continue;
                }
                current.push('\\');
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
        i += 1;
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::FileList;
    use crate::registry::{Category, Check, CheckKind, RunPolicy, Scope};
    use std::path::PathBuf;

    #[test]
    fn inject_config_inserts_after_binary() {
        let argv = vec!["shellcheck".to_string(), "file.sh".to_string()];
        let config = vec!["--rcfile".to_string(), "/cfg/.shellcheckrc".to_string()];
        assert_eq!(
            inject_config(argv, &config),
            vec!["shellcheck", "--rcfile", "/cfg/.shellcheckrc", "file.sh"],
        );
    }

    #[test]
    fn inject_config_noop_when_no_config_args() {
        let argv = vec!["shellcheck".to_string(), "file.sh".to_string()];
        assert_eq!(inject_config(argv.clone(), &[]), argv,);
    }

    #[test]
    fn inject_config_noop_when_argv_empty() {
        assert_eq!(
            inject_config(vec![], &["--rcfile".to_string()]),
            vec![] as Vec<String>
        );
    }

    #[test]
    fn resolve_linter_config_absent_file_returns_empty() {
        let check = Check::file("shellcheck", "shellcheck {FILE}", &["*.sh"])
            .linter_config(".shellcheckrc", "--rcfile");
        let dir = tempfile::tempdir().unwrap();
        assert!(resolve_linter_config(&check, dir.path()).is_empty());
    }

    #[test]
    fn resolve_linter_config_present_file_returns_flag_and_path() {
        let check = Check::file("shellcheck", "shellcheck {FILE}", &["*.sh"])
            .linter_config(".shellcheckrc", "--rcfile");
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join(".shellcheckrc");
        std::fs::write(&cfg_path, "").unwrap();
        let result = resolve_linter_config(&check, dir.path());
        assert_eq!(
            result,
            vec!["--rcfile", cfg_path.to_string_lossy().as_ref()]
        );
    }

    #[test]
    fn resolve_linter_config_none_returns_empty() {
        let check = Check::file("shellcheck", "shellcheck {FILE}", &["*.sh"]);
        let dir = tempfile::tempdir().unwrap();
        assert!(resolve_linter_config(&check, dir.path()).is_empty());
    }

    #[test]
    fn render_config_args_shell_quotes_all_args() {
        let rendered =
            render_config_args(&["--config-path".to_string(), "/tmp/my cfg".to_string()]);
        assert_eq!(rendered, "\"--config-path\" \"/tmp/my cfg\"");
    }

    #[test]
    fn resolve_linter_config_dir_if_any_returns_flag_and_dir() {
        let check = Check::file("biome", "biome check {FILE}", &["*.json"])
            .linter_config_dir_if_any(&["biome.jsonc"], "--config-path");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("biome.jsonc"), "{}").unwrap();
        let result = resolve_linter_config(&check, dir.path());
        assert_eq!(
            result,
            vec![
                "--config-path".to_string(),
                dir.path().to_string_lossy().into_owned()
            ]
        );
    }

    fn project_check(patterns: &'static [&'static str]) -> Check {
        Check {
            name: "test",
            bin_name: "test-bin",
            mise_tool_name: None,
            version_range: None,
            patterns,
            excludes_if_active: &[],
            linter_config: None,
            env: &[],
            nonverbose_filter_prefixes: &[],
            stderr_filter_prefixes: &[],
            baseline_config: None,
            unsupported_configs: &[],
            tool_key_migrations: vec![],
            is_formatter: false,
            defers_to_formatters: false,
            editorconfig_line_length_policy: crate::registry::EditorconfigLineLengthPolicy::Default,
            activate_unconditionally: false,
            category: Category::Default,
            run_policy: RunPolicy::Fast,
            toolchain: None,
            windows_java_jar: false,
            fix_behavior: crate::registry::FixBehavior::Definitive,
            kind: CheckKind::Template {
                check_cmd: "run-it",
                fix_cmd: "",
                full_cmd: "",
                full_fix_cmd: "",
                scope: Scope::Project,
            },
            desc: "",
            docs: "",
        }
    }

    fn file_list(paths: &[&str]) -> FileList {
        FileList {
            files: paths
                .iter()
                .map(|s| PathBuf::from(format!("/repo/{s}")))
                .collect(),
            changed_paths: paths.iter().map(|path| path.to_string()).collect(),
            merge_base: Some("abc123".to_string()),
            full: false,
        }
    }

    #[test]
    fn project_scope_skips_when_no_matching_files() {
        let check = project_check(&["*.rs"]);
        let fl = file_list(&["foo.py", "bar.md"]);
        assert!(
            build_invocations(
                &check,
                &fl,
                false,
                Path::new("/repo"),
                &[],
                Path::new("/repo"),
            )
            .is_empty()
        );
    }

    #[test]
    fn project_scope_runs_when_matching_files_present() {
        let check = project_check(&["*.rs"]);
        let fl = file_list(&["src/main.rs", "foo.py"]);
        let inv = build_invocations(
            &check,
            &fl,
            false,
            Path::new("/repo"),
            &[],
            Path::new("/repo"),
        );
        assert_eq!(inv, vec![vec!["run-it".to_string()]]);
    }

    #[test]
    fn project_scope_empty_patterns_always_runs() {
        let check = project_check(&[]);
        let fl = file_list(&["foo.py"]);
        let inv = build_invocations(
            &check,
            &fl,
            false,
            Path::new("/repo"),
            &[],
            Path::new("/repo"),
        );
        assert_eq!(inv, vec![vec!["run-it".to_string()]]);
    }

    #[test]
    fn appends_rust_component_note_for_missing_clippy() {
        let mut stderr = b"error: 'cargo-clippy' is not installed for the toolchain '1.94.1-x86_64-unknown-linux-gnu'.\n".to_vec();

        maybe_append_rust_component_note("cargo-clippy", &mut stderr);

        let msg = String::from_utf8(stderr).unwrap();
        assert!(msg.contains("NOTE: `cargo-clippy` needs the Rust `clippy` component"));
        assert!(msg.contains("rustup component add clippy"));
    }

    #[test]
    fn appends_rust_component_note_for_missing_rustfmt() {
        let mut stderr =
            b"error: 'rustfmt' is not installed for the toolchain '1.94.1-x86_64-unknown-linux-gnu'.\n".to_vec();

        maybe_append_rust_component_note("cargo-fmt", &mut stderr);

        let msg = String::from_utf8(stderr).unwrap();
        assert!(msg.contains("NOTE: `cargo-fmt` needs the Rust `rustfmt` component"));
        assert!(msg.contains("rustup component add rustfmt"));
    }

    #[test]
    fn filters_matching_stderr_prefixes() {
        let stderr = b" INFO taplo: noisy\nERROR useful\n";
        let filtered = filter_stderr_lines(stderr, &[" INFO taplo:"]);
        assert_eq!(String::from_utf8(filtered).unwrap(), "ERROR useful\n");
    }

    #[test]
    fn preserves_non_matching_stderr_lines() {
        let stderr = b"ERROR useful\n";
        let filtered = filter_stderr_lines(stderr, &[" INFO taplo:"]);
        assert_eq!(String::from_utf8(filtered).unwrap(), "ERROR useful\n");
    }

    #[test]
    fn filters_rumdl_success_lines_from_nonverbose_output() {
        let output =
            b"Success: No issues found in 1 file (8ms)\nerror[MD013]: too long\n".as_slice();
        let filtered = filter_output_lines(output, |line| {
            line.starts_with("Success: No issues found in ")
        });
        assert_eq!(
            String::from_utf8(filtered).unwrap(),
            "error[MD013]: too long\n"
        );
    }

    #[test]
    fn preserves_non_success_rumdl_lines() {
        let output = b"warning: keep me\n".as_slice();
        let filtered = filter_output_lines(output, |line| {
            line.starts_with("Success: No issues found in ")
        });
        assert_eq!(String::from_utf8(filtered).unwrap(), "warning: keep me\n");
    }
}
