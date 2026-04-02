use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;

pub struct FileList {
    pub files: Vec<PathBuf>,
    /// The merge base ref, used by project-scoped checks (e.g. golangci-lint).
    pub merge_base: Option<String>,
}

pub fn changed(
    project_root: &Path,
    cfg: &Config,
    full: bool,
    from_ref: Option<&str>,
    to_ref: Option<&str>,
) -> Result<FileList> {
    if full {
        return all_files(project_root, cfg);
    }

    // Determine merge base.
    let merge_base = resolve_merge_base(project_root, cfg, from_ref)?;

    let files = if let Some(ref base) = merge_base {
        let to = to_ref.unwrap_or("HEAD");
        collect_changed_files(project_root, cfg, base, to)?
    } else {
        // No merge base (shallow clone etc.) — fall back to all files.
        return all_files(project_root, cfg);
    };

    Ok(FileList { files, merge_base })
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

fn collect_changed_files(
    project_root: &Path,
    cfg: &Config,
    base: &str,
    to: &str,
) -> Result<Vec<PathBuf>> {
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

    Ok(filter_existing(project_root, cfg, names))
}

fn all_files(project_root: &Path, cfg: &Config) -> Result<FileList> {
    let out = Command::new("git")
        .args(["ls-files"])
        .current_dir(project_root)
        .output()
        .context("git ls-files")?;

    let names: std::collections::BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();

    Ok(FileList {
        files: filter_existing(project_root, cfg, names),
        merge_base: None,
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
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

fn filter_existing(
    project_root: &Path,
    cfg: &Config,
    names: std::collections::BTreeSet<String>,
) -> Vec<PathBuf> {
    let exclude_re: Option<regex::Regex> = cfg
        .settings
        .exclude
        .as_deref()
        .and_then(|pat| regex::Regex::new(pat).ok());

    names
        .into_iter()
        .filter(|name| {
            if let Some(re) = &exclude_re {
                !re.is_match(name)
            } else {
                true
            }
        })
        .map(|name| project_root.join(&name))
        .filter(|p| p.exists())
        .collect()
}
