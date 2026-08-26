// Check ordering, file selection, and command-line construction for the runner.

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::files::{FileList, match_files};
use crate::registry::{Check, CheckKind, LinterConfig, NativePrepareContext, Scope};

use super::{PreparedCheck, quote_path, shell_words, substitute_merge_base};

pub(super) fn order_checks_for_fix<'a>(checks: &[&'a Check]) -> Result<Vec<&'a Check>> {
    let mut remaining = checks.to_vec();
    remaining.sort_by_key(|check| !check.fix_first);
    let mut ordered = Vec::with_capacity(remaining.len());

    while !remaining.is_empty() {
        let Some(index) = remaining.iter().position(|check| {
            check.fix_after.iter().all(|dependency| {
                !remaining
                    .iter()
                    .any(|candidate| candidate.name == *dependency)
            })
        }) else {
            let names = remaining
                .iter()
                .map(|check| check.name)
                .collect::<Vec<_>>()
                .join(", ");
            bail!("cyclic fixer ordering involving: {names}");
        };
        ordered.push(remaining.remove(index));
    }

    Ok(ordered)
}

pub(super) fn prepare(
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
                java_jar: check
                    .java_jar
                    .is_some_and(|mode| mode.enabled_on_current_platform()),
                env: check.env,
                nonverbose_filter_prefixes: check.nonverbose_filter_prefixes,
                stderr_filter_prefixes: check.stderr_filter_prefixes,
                failure_output_patterns: check.failure_output_patterns,
                nonverbose_failure_output: check.nonverbose_failure_output,
                missing_component_hint: check.missing_component_hint,
            })
        }
        CheckKind::Native(native) => native
            .prepare(NativePrepareContext {
                name: check.name,
                file_list,
                project_root,
                cfg,
                config_dir,
            })
            .map(PreparedCheck::Native),
    }
}

pub(super) fn tracked_files(
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
pub(super) fn build_invocations(
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
            chunk_files_by_length(&matched, project_root)
                .into_iter()
                .map(|chunk| {
                    let files_arg: String = chunk
                        .iter()
                        .map(|f| quote_path(f))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let rel_files_arg: String = chunk
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
                    if inject_config_args {
                        inject_config(argv, &config_args)
                    } else {
                        argv
                    }
                })
                .collect()
        }
    }
}

/// Maximum characters to put in a single `{FILES}`/`{RELFILES}` substitution. Chosen well under
/// cmd.exe's ~8191-char command-line buffer — some `Scope::Files` checks are mise shims that
/// `spawn_command` routes through `cmd.exe /C` on Windows, which is a tighter limit than
/// `CreateProcessW`'s ~32KB — leaving headroom for the command template itself, quoting, and the
/// other substitutions applied afterward (`{CONFIG_ARGS}`, `{CARGO_EDITION_FLAG}`).
pub(super) const MAX_FILES_ARG_CHARS: usize = 6000;

/// Splits `files` into groups whose rendered `{FILES}`/`{RELFILES}` argument stays under
/// [`MAX_FILES_ARG_CHARS`], so `Scope::Files` checks issue multiple invocations instead of one
/// unbounded one. Without this, a large PR (or full run) touching many/long-path files could
/// overflow Windows' command-line length limits — see the `git check-attr` bug this pattern was
/// extracted from.
pub(super) fn chunk_files_by_length<'a>(
    files: &[&'a PathBuf],
    project_root: &Path,
) -> Vec<Vec<&'a PathBuf>> {
    let mut chunks: Vec<Vec<&PathBuf>> = vec![];
    let mut current: Vec<&PathBuf> = vec![];
    let mut current_len = 0usize;

    for f in files.iter().copied() {
        // Use the longer of the absolute/quoted-relative representations as a conservative
        // per-file length estimate, since the template may substitute either {FILES} or
        // {RELFILES} (or both).
        let abs_len = quote_path(f).len();
        let rel_len = quote_path(f.strip_prefix(project_root).unwrap_or(f)).len();
        let len = abs_len.max(rel_len) + 1; // +1 for the joining space

        if !current.is_empty() && current_len + len > MAX_FILES_ARG_CHARS {
            chunks.push(std::mem::take(&mut current));
            current_len = 0;
        }
        current.push(f);
        current_len += len;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Returns `--edition <edition>` if a Rust edition is declared in the project's
/// `Cargo.toml`, or an empty string if not found. Used to substitute
/// `{CARGO_EDITION_FLAG}` in rustfmt command templates.
pub(super) fn resolve_cargo_edition_flag(project_root: &Path) -> String {
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
pub(super) fn resolve_linter_config(check: &Check, config_dir: &Path) -> Vec<String> {
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
pub(super) fn inject_config(mut argv: Vec<String>, config_args: &[String]) -> Vec<String> {
    if config_args.is_empty() || argv.is_empty() {
        return argv;
    }
    // Insert after argv[0] (the binary name).
    let tail = argv.split_off(1);
    argv.extend_from_slice(config_args);
    argv.extend(tail);
    argv
}

pub(super) fn render_config_args(config_args: &[String]) -> String {
    config_args
        .iter()
        .map(|arg| quote_path(Path::new(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}
