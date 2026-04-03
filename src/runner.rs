use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio::task::JoinSet;

use crate::config::{Config, LycheeConfig, RenovateDepsConfig};
use crate::files::FileList;
use crate::linters::{lychee, renovate_deps};
use crate::registry::{Check, CheckKind, Scope, SpecialKind};

pub struct RunOptions {
    pub fix: bool,
    pub verbose: bool,
    pub short: bool,
}

pub struct CheckResult {
    pub name: String,
    pub ok: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// A check with all inputs pre-resolved, ready to execute without borrowing
/// the registry or config. Built by `prepare()` before the fix/check split.
enum PreparedCheck {
    Invocations {
        name: String,
        argv_list: Vec<Vec<String>>,
    },
    Links {
        name: String,
        cfg: LycheeConfig,
        file_list: FileList,
        config_dir: PathBuf,
    },
    RenovateDeps {
        name: String,
        cfg: RenovateDepsConfig,
    },
}

impl PreparedCheck {
    fn name(&self) -> &str {
        match self {
            Self::Invocations { name, .. }
            | Self::Links { name, .. }
            | Self::RenovateDeps { name, .. } => name,
        }
    }

    async fn execute(self, fix: bool, project_root: &Path) -> CheckResult {
        let name = self.name().to_string();
        let (ok, stdout, stderr) = match self {
            Self::Invocations { argv_list, .. } => {
                run_invocations(&name, &argv_list, project_root).await
            }
            Self::Links {
                cfg,
                file_list,
                config_dir,
                ..
            } => lychee::run(&cfg, &file_list, project_root, &config_dir).await,
            Self::RenovateDeps { cfg, .. } => renovate_deps::run(&cfg, fix, project_root).await,
        };
        CheckResult {
            name,
            ok,
            stdout,
            stderr,
        }
    }
}

pub async fn run(
    checks: &[&Check],
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
    } = opts;
    let prepared: Vec<PreparedCheck> = checks
        .iter()
        .filter_map(|&check| prepare(check, file_list, fix, project_root, checks, cfg, config_dir))
        .collect();

    if fix {
        let mut results = vec![];
        for task in prepared {
            let r = task.execute(fix, project_root).await;
            if !short && (verbose || !r.ok) {
                eprintln!("[{}]", r.name);
                flush_output(&r.stdout, &r.stderr);
            }
            results.push(r);
        }
        return Ok(results);
    }

    let mut set: JoinSet<CheckResult> = JoinSet::new();
    for task in prepared {
        let root = project_root.to_path_buf();
        set.spawn(async move {
            let r = task.execute(false, &root).await;
            if verbose {
                eprintln!("[{}]", r.name);
                flush_output(&r.stdout, &r.stderr);
            }
            r
        });
    }

    // Collect all results before printing to avoid interleaved output.
    let mut collected = vec![];
    while let Some(res) = set.join_next().await {
        collected.push(res?);
    }

    if !verbose && !short {
        for r in &collected {
            if !r.ok {
                eprintln!("[{}]", r.name);
                flush_output(&r.stdout, &r.stderr);
            }
        }
    }

    Ok(collected)
}

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
            let argv_list = build_invocations(check, file_list, fix, project_root, active_checks);
            if argv_list.is_empty() {
                return None;
            }
            Some(PreparedCheck::Invocations { name, argv_list })
        }
        CheckKind::Special(SpecialKind::Links) => Some(PreparedCheck::Links {
            name,
            cfg: cfg.checks.lychee.clone(),
            file_list: file_list.clone(),
            config_dir: config_dir.to_path_buf(),
        }),
        CheckKind::Special(SpecialKind::RenovateDeps) => Some(PreparedCheck::RenovateDeps {
            name,
            cfg: cfg.checks.renovate_deps.clone(),
        }),
    }
}

/// Returns the list of argv vectors to execute for a check.
fn build_invocations(
    check: &Check,
    file_list: &FileList,
    fix: bool,
    project_root: &Path,
    active_checks: &[&Check],
) -> Vec<Vec<String>> {
    let CheckKind::Template {
        check_cmd,
        fix_cmd,
        scope,
    } = &check.kind
    else {
        return vec![];
    };

    let cmd_template = if fix && check.has_fix() {
        fix_cmd
    } else {
        check_cmd
    };

    // Collect patterns from checks that are active and listed in excludes_if_active.
    let excludes: Vec<&str> = active_checks
        .iter()
        .filter(|c| check.excludes_if_active.contains(&c.name))
        .flat_map(|c| c.patterns.iter().copied())
        .collect();

    match scope {
        Scope::Project => {
            // If patterns are set, only run when relevant files are present.
            if !check.patterns.is_empty()
                && match_files(&file_list.files, check.patterns, &excludes, project_root).is_empty()
            {
                return vec![];
            }
            let cmd = substitute_merge_base(cmd_template, file_list.merge_base.as_deref());
            vec![shell_words(cmd)]
        }

        Scope::File => {
            let matched = match_files(&file_list.files, check.patterns, &excludes, project_root);
            matched
                .iter()
                .map(|f| {
                    let cmd = cmd_template.replace("{FILE}", &quote_path(f));
                    shell_words(cmd)
                })
                .collect()
        }

        Scope::Files => {
            let matched = match_files(&file_list.files, check.patterns, &excludes, project_root);
            if matched.is_empty() {
                return vec![];
            }
            let files_arg: String = matched
                .iter()
                .map(|f| quote_path(f))
                .collect::<Vec<_>>()
                .join(" ");
            let cmd = cmd_template.replace("{FILES}", &files_arg);
            vec![shell_words(cmd)]
        }
    }
}

/// Runs all invocations for one check, returning (ok, stdout, stderr).
/// Never prints — callers decide when and whether to flush output.
async fn run_invocations(
    name: &str,
    invocations: &[Vec<String>],
    root: &Path,
) -> (bool, Vec<u8>, Vec<u8>) {
    let mut all_ok = true;
    let mut combined_stdout = Vec::new();
    let mut combined_stderr = Vec::new();

    for argv in invocations {
        if argv.is_empty() {
            continue;
        }
        let result = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .await;
        match result {
            Ok(out) => {
                combined_stdout.extend_from_slice(&out.stdout);
                combined_stderr.extend_from_slice(&out.stderr);
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

    (all_ok, combined_stdout, combined_stderr)
}

fn flush_output(stdout: &[u8], stderr: &[u8]) {
    if !stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(stdout));
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
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn shell_words(cmd: String) -> Vec<String> {
    // Minimal word-splitting that respects single-quoted strings.
    let mut words = vec![];
    let mut current = String::new();
    let mut in_single = false;
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\'' if !in_single => {
                in_single = true;
            }
            '\'' if in_single => {
                in_single = false;
            }
            ' ' | '\t' if !in_single => {
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
    use crate::registry::{Check, CheckKind, Scope};
    use std::path::PathBuf;

    fn project_check(patterns: &'static [&'static str]) -> Check {
        Check {
            name: "test",
            bin_name: "test-bin",
            mise_tool_name: None,
            version_range: None,
            patterns,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "run-it",
                fix_cmd: "",
                scope: Scope::Project,
            },
        }
    }

    fn file_list(paths: &[&str]) -> FileList {
        FileList {
            files: paths
                .iter()
                .map(|s| PathBuf::from(format!("/repo/{s}")))
                .collect(),
            merge_base: Some("abc123".to_string()),
        }
    }

    #[test]
    fn project_scope_skips_when_no_matching_files() {
        let check = project_check(&["*.rs"]);
        let fl = file_list(&["foo.py", "bar.md"]);
        assert!(build_invocations(&check, &fl, false, Path::new("/repo"), &[]).is_empty());
    }

    #[test]
    fn project_scope_runs_when_matching_files_present() {
        let check = project_check(&["*.rs"]);
        let fl = file_list(&["src/main.rs", "foo.py"]);
        let inv = build_invocations(&check, &fl, false, Path::new("/repo"), &[]);
        assert_eq!(inv, vec![vec!["run-it".to_string()]]);
    }

    #[test]
    fn project_scope_empty_patterns_always_runs() {
        let check = project_check(&[]);
        let fl = file_list(&["foo.py"]);
        let inv = build_invocations(&check, &fl, false, Path::new("/repo"), &[]);
        assert_eq!(inv, vec![vec!["run-it".to_string()]]);
    }
}
