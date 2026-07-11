use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::{BTreeSet, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
            filter_names(project_root, &exclude, names.clone())?,
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
    for line in git_diff_names(project_root, &[&range])? {
        names.insert(line);
    }
    // Unstaged changes.
    for line in git_diff_names(project_root, &[])? {
        names.insert(line);
    }
    // Staged changes.
    for line in git_diff_names(project_root, &["--cached"])? {
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

    let names: BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();

    Ok(FileList {
        files: filter_names(project_root, exclude, names)?,
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
    names: BTreeSet<String>,
) -> Result<Vec<PathBuf>> {
    let candidates: BTreeSet<String> = names
        .into_iter()
        .filter(|name| !BUILTIN_EXCLUDES.contains(&name.as_str()))
        .filter(|name| !exclude.is_match(name))
        .collect();
    let generated = generated_paths(project_root, &candidates)?;

    Ok(candidates
        .into_iter()
        .filter(|name| !generated.contains(name))
        .map(|name| project_root.join(name))
        .filter(|path| path.exists())
        .collect())
}

fn generated_paths(project_root: &Path, names: &BTreeSet<String>) -> Result<HashSet<String>> {
    if names.is_empty() {
        return Ok(HashSet::new());
    }

    // Feed paths via stdin (`--stdin -z`) instead of as CLI args. Passing many/long paths as
    // argv batches hit Windows' ~32KB CreateProcess command-line limit on repos with long paths
    // (os error 206, ERROR_FILENAME_EXCED_RANGE) even with modest batch sizes; stdin has no such
    // limit on any platform.
    let mut child = Command::new("git")
        .args(["check-attr", "--stdin", "-z", "linguist-generated"])
        .current_dir(project_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("git check-attr")?;

    // Write on a separate thread: git may start writing output before we finish writing input,
    // and the stdout pipe can fill up (typical OS pipe buffers are 4-64KB) well before all paths
    // are written for a large repo, which would deadlock if done sequentially on this thread.
    let mut stdin = child.stdin.take().expect("stdin was piped");
    let names_for_writer: Vec<String> = names.iter().cloned().collect();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for name in &names_for_writer {
            stdin.write_all(name.as_bytes())?;
            stdin.write_all(b"\0")?;
        }
        Ok(())
    });

    let out = child.wait_with_output().context("git check-attr")?;
    writer
        .join()
        .expect("git check-attr stdin writer thread panicked")
        .context("git check-attr: writing stdin")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("git check-attr failed ({}): {}", out.status, stderr.trim());
    }

    let mut generated = HashSet::new();
    let fields: Vec<&[u8]> = out.stdout.split(|byte| *byte == 0).collect();
    for chunk in fields.chunks_exact(3) {
        let path = String::from_utf8_lossy(chunk[0]);
        let info = String::from_utf8_lossy(chunk[2]);
        if matches!(info.as_ref(), "set" | "true") {
            generated.insert(path.into_owned());
        }
    }

    Ok(generated)
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
        let out = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "git init failed");
        std::fs::write(tmp.path().join("present.md"), "ok\n").unwrap();
        let names = ["missing.md".to_string(), "present.md".to_string()]
            .into_iter()
            .collect();

        let files =
            filter_names(tmp.path(), &GlobSetBuilder::new().build().unwrap(), names).unwrap();

        assert_eq!(files, vec![tmp.path().join("present.md")]);
    }

    #[test]
    fn filter_names_skips_generated_paths_from_gitattributes() {
        let tmp = tempfile::TempDir::new().unwrap();

        for args in [
            ["init", "-b", "main"].as_slice(),
            ["config", "user.email", "test@test.com"].as_slice(),
            ["config", "user.name", "Test"].as_slice(),
        ] {
            let out = Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .unwrap();
            assert!(out.status.success(), "git {:?} failed", args);
        }

        std::fs::write(
            tmp.path().join(".gitattributes"),
            "generated.sh linguist-generated\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("generated.sh"), "#!/bin/sh\n").unwrap();
        std::fs::write(tmp.path().join("custom.txt"), "not excluded\n").unwrap();
        std::fs::write(tmp.path().join("tracked.sh"), "#!/bin/sh\n").unwrap();

        let names = ["custom.txt", "generated.sh", "tracked.sh"]
            .into_iter()
            .map(str::to_string)
            .collect();

        let files =
            filter_names(tmp.path(), &GlobSetBuilder::new().build().unwrap(), names).unwrap();

        assert_eq!(
            files,
            vec![tmp.path().join("custom.txt"), tmp.path().join("tracked.sh")]
        );
    }

    #[test]
    fn filter_names_skips_true_generated_paths_from_gitattributes() {
        let tmp = tempfile::TempDir::new().unwrap();

        for args in [
            ["init", "-b", "main"].as_slice(),
            ["config", "user.email", "test@test.com"].as_slice(),
            ["config", "user.name", "Test"].as_slice(),
        ] {
            let out = Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .unwrap();
            assert!(out.status.success(), "git {:?} failed", args);
        }

        std::fs::write(
            tmp.path().join(".gitattributes"),
            "generated.sh linguist-generated=true\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("generated.sh"), "#!/bin/sh\n").unwrap();
        std::fs::write(tmp.path().join("tracked.sh"), "#!/bin/sh\n").unwrap();

        let names = ["generated.sh", "tracked.sh"]
            .into_iter()
            .map(str::to_string)
            .collect();

        let files =
            filter_names(tmp.path(), &GlobSetBuilder::new().build().unwrap(), names).unwrap();

        assert_eq!(files, vec![tmp.path().join("tracked.sh")]);
    }

    /// Regression test for os error 206 (ERROR_FILENAME_EXCED_RANGE) on Windows: the previous
    /// implementation passed paths as CLI args in batches of 256, which overflowed Windows'
    /// ~32KB CreateProcess command-line limit for repos with many/long paths (this is exactly
    /// what broke on a real large checkout — see the PR this test was added in). Names here are
    /// long enough that a single 256-path argv batch would total well over 32KB, so this test
    /// would fail with os error 206 under the old implementation when run on Windows — this
    /// crate's CI matrix (.github/workflows/test.yml) already runs `cargo test` on windows-2025,
    /// so this test exercises the real failure mode there, not just on Linux/macOS where argv
    /// limits are much higher and wouldn't have caught this. Also verifies large-N correctness
    /// and that writing stdin doesn't deadlock against a filled stdout pipe.
    #[test]
    fn filter_names_handles_many_long_paths_via_stdin() {
        let tmp = tempfile::TempDir::new().unwrap();

        for args in [
            ["init", "-b", "main"].as_slice(),
            ["config", "user.email", "test@test.com"].as_slice(),
            ["config", "user.name", "Test"].as_slice(),
        ] {
            let out = Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .unwrap();
            assert!(out.status.success(), "git {:?} failed", args);
        }

        std::fs::write(
            tmp.path().join(".gitattributes"),
            "*.generated.txt linguist-generated=true\n",
        )
        .unwrap();

        // Long names (well under the 255-char filesystem component limit) so even a single
        // argv-based batch of 256 paths would total ~256 * 220 chars ~= 56KB — comfortably over
        // Windows' ~32KB command-line limit, not just barely over it.
        let long_prefix = "a".repeat(200);
        let mut names = BTreeSet::new();
        let mut expected_kept = Vec::new();
        for i in 0..2000 {
            let generated_name = format!("{long_prefix}-{i}.generated.txt");
            std::fs::write(tmp.path().join(&generated_name), "generated\n").unwrap();
            names.insert(generated_name);

            let kept_name = format!("{long_prefix}-{i}.txt");
            std::fs::write(tmp.path().join(&kept_name), "kept\n").unwrap();
            names.insert(kept_name.clone());
            expected_kept.push(tmp.path().join(kept_name));
        }
        expected_kept.sort();

        let mut files =
            filter_names(tmp.path(), &GlobSetBuilder::new().build().unwrap(), names).unwrap();
        files.sort();

        assert_eq!(files, expected_kept);
    }
}
