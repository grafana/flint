use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio::task::JoinSet;

use crate::files::FileList;
use crate::registry::{Check, Scope};

pub async fn run(
    checks: &[&Check],
    file_list: &FileList,
    fix: bool,
    project_root: &Path,
) -> Result<Vec<(String, bool)>> {
    let mut set: JoinSet<(String, bool)> = JoinSet::new();

    for &check in checks {
        let invocations = build_invocations(check, file_list, fix, project_root);
        if invocations.is_empty() {
            continue;
        }

        let name = check.name.to_string();
        let root = project_root.to_path_buf();

        set.spawn(async move {
            let ok = run_invocations(&name, &invocations, &root).await;
            (name, ok)
        });
    }

    let mut results = vec![];
    while let Some(res) = set.join_next().await {
        results.push(res?);
    }

    Ok(results)
}

/// Returns the list of argv vectors to execute for a check.
fn build_invocations(
    check: &Check,
    file_list: &FileList,
    fix: bool,
    project_root: &Path,
) -> Vec<Vec<String>> {
    let cmd_template = if fix && check.has_fix() {
        check.fix_cmd
    } else {
        check.check_cmd
    };

    match check.scope {
        Scope::Project => {
            let cmd = substitute_merge_base(cmd_template, file_list.merge_base.as_deref());
            vec![shell_words(cmd)]
        }

        Scope::File => {
            let patterns: Vec<&str> = check.patterns.split_whitespace().collect();
            let matched = match_files(&file_list.files, &patterns, project_root);
            matched
                .iter()
                .map(|f| {
                    let cmd = cmd_template.replace("{FILE}", &quote_path(f));
                    shell_words(cmd)
                })
                .collect()
        }

        Scope::Files => {
            let patterns: Vec<&str> = check.patterns.split_whitespace().collect();
            let matched = match_files(&file_list.files, &patterns, project_root);
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

async fn run_invocations(name: &str, invocations: &[Vec<String>], root: &Path) -> bool {
    let mut all_ok = true;
    for argv in invocations {
        if argv.is_empty() {
            continue;
        }
        let status = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(root)
            .stdin(Stdio::null())
            .status()
            .await;
        match status {
            Ok(s) if s.success() => {}
            Ok(_) => {
                all_ok = false;
            }
            Err(e) => {
                eprintln!("flint: {name}: failed to spawn: {e}");
                all_ok = false;
            }
        }
    }
    all_ok
}

fn match_files<'a>(
    files: &'a [PathBuf],
    patterns: &[&str],
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
            patterns.iter().any(|pat| {
                if *pat == "*" {
                    return true;
                }
                glob_match(pat, file_name.as_ref()) || glob_match(pat, rel_str.as_ref())
            })
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
    // Simple single-quote escaping.
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
