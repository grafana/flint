use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::linters::renovate_deps::COMMITTED_PATHS;

/// Files managed by flint itself — always excluded from generic linter checks.
const BUILTIN_EXCLUDES: &[&str] = COMMITTED_PATHS;

#[derive(Debug, Clone)]
pub struct FileList {
    pub files: Vec<PathBuf>,
    /// Changed paths from git before user excludes are applied.
    pub changed_paths: Vec<String>,
    /// The merge base ref, used by project-scoped checks (e.g. golangci-lint).
    pub merge_base: Option<String>,
    /// True when the file list contains all project files (explicit --full or no merge base).
    /// Used by checks with a `full_cmd` to switch to a project-wide command.
    pub full: bool,
}

pub fn changed(
    project_root: &Path,
    cfg: &Config,
    full: bool,
    from_ref: Option<&str>,
    to_ref: Option<&str>,
) -> Result<FileList> {
    let exclude = build_exclude_set(cfg);

    if full {
        return all(project_root, cfg);
    }

    let merge_base = resolve_merge_base(project_root, cfg, from_ref)?;

    let (files, changed_paths) = if let Some(ref base) = merge_base {
        let to = to_ref.unwrap_or("HEAD");
        let names = collect_changed_names(project_root, base, to)?;
        (
            filter_names(project_root, &exclude, names.clone()),
            names.into_iter().collect(),
        )
    } else {
        // No merge base (shallow clone etc.) — fall back to all files.
        return all(project_root, cfg);
    };

    Ok(FileList {
        files,
        changed_paths,
        merge_base,
        full: false,
    })
}

fn build_exclude_set(cfg: &Config) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in &cfg.settings.exclude {
        match GlobBuilder::new(pattern).literal_separator(true).build() {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(e) => {
                eprintln!("flint: invalid exclude pattern {pattern:?}: {e}");
            }
        }
    }
    builder.build().unwrap_or_default()
}

fn resolve_merge_base(
    project_root: &Path,
    cfg: &Config,
    from_ref: Option<&str>,
) -> Result<Option<String>> {
    let base_ref = from_ref.unwrap_or(cfg.settings.base_branch.as_str());

    // Try `origin/<base>` first, then bare `<base>`.
    for candidate in [format!("origin/{base_ref}"), base_ref.to_string()] {
        let out = Command::new("git")
            .args(["merge-base", &candidate, "HEAD"])
            .current_dir(project_root)
            .output()
            .context("git merge-base")?;
        if out.status.success() {
            return Ok(Some(
                String::from_utf8_lossy(&out.stdout).trim().to_string(),
            ));
        }
    }

    Ok(None)
}

fn collect_changed_names(
    project_root: &Path,
    base: &str,
    to: &str,
) -> Result<std::collections::BTreeSet<String>> {
    let range = format!("{base}...{to}");
    let mut names: std::collections::BTreeSet<String> = Default::default();

    // Committed changes in the range.
    for line in git_diff_names(project_root, &["--diff-filter=d", &range])? {
        names.insert(line);
    }
    // Unstaged changes.
    for line in git_diff_names(project_root, &["--diff-filter=d"])? {
        names.insert(line);
    }
    // Staged changes.
    for line in git_diff_names(project_root, &["--cached", "--diff-filter=d"])? {
        names.insert(line);
    }

    Ok(names)
}

pub fn all(project_root: &Path, cfg: &Config) -> Result<FileList> {
    let exclude = build_exclude_set(cfg);
    all_files(project_root, &exclude)
}

fn all_files(project_root: &Path, exclude: &GlobSet) -> Result<FileList> {
    let out = Command::new("git")
        .args(["ls-files"])
        .current_dir(project_root)
        .output()
        .context("git ls-files")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("git ls-files failed ({}): {}", out.status, stderr.trim());
    }

    let names: std::collections::BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();

    Ok(FileList {
        files: filter_names(project_root, exclude, names),
        changed_paths: vec![],
        merge_base: None,
        full: true,
    })
}

fn git_diff_names(project_root: &Path, extra_args: &[&str]) -> Result<Vec<String>> {
    let mut args = vec!["diff", "--name-only"];
    args.extend_from_slice(extra_args);
    let out = Command::new("git")
        .args(&args)
        .current_dir(project_root)
        .output()
        .context("git diff --name-only")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!(
            "git diff --name-only failed ({}): {}",
            out.status,
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

fn filter_names(
    project_root: &Path,
    exclude: &GlobSet,
    names: std::collections::BTreeSet<String>,
) -> Vec<PathBuf> {
    names
        .into_iter()
        .filter(|name| !BUILTIN_EXCLUDES.contains(&name.as_str()))
        .filter(|name| !exclude.is_match(name))
        .map(|name| project_root.join(name))
        .filter(|path| path.exists())
        .collect()
}

pub fn match_files<'a>(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_names_skips_deleted_worktree_paths() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("present.md"), "ok\n").unwrap();
        let names = ["missing.md".to_string(), "present.md".to_string()]
            .into_iter()
            .collect();

        let files = filter_names(tmp.path(), &GlobSetBuilder::new().build().unwrap(), names);

        assert_eq!(files, vec![tmp.path().join("present.md")]);
    }
}
