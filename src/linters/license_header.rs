use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::config::LicenseHeaderConfig;
use crate::linters::LinterOutput;

/// Checks that each file matching `cfg.patterns` contains `cfg.text` within
/// the first `cfg.lines_to_check` lines. Returns early (ok=true) when not configured.
pub async fn run(
    cfg: &LicenseHeaderConfig,
    project_root: &Path,
    files: &[PathBuf],
) -> LinterOutput {
    if cfg.text.is_empty() {
        return LinterOutput::ok();
    }

    let mut all_ok = true;
    let mut stderr = Vec::new();

    for file in files {
        let rel = file.strip_prefix(project_root).unwrap_or(file);
        let rel_str = rel.to_string_lossy();
        let file_name = file
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        if !cfg
            .patterns
            .iter()
            .any(|pat| glob_match(pat, &file_name) || glob_match(pat, &rel_str))
        {
            continue;
        }

        match check_file(file, &cfg.text, cfg.lines_to_check) {
            Ok(true) => {}
            Ok(false) => {
                all_ok = false;
                stderr.extend_from_slice(format!("{rel_str}: missing license header\n").as_bytes());
            }
            Err(e) => {
                all_ok = false;
                stderr.extend_from_slice(format!("{rel_str}: failed to read: {e}\n").as_bytes());
            }
        }
    }

    LinterOutput {
        ok: all_ok,
        stdout: vec![],
        stderr,
    }
}

/// Returns `true` if `text` appears anywhere within the first `lines_to_check` lines of `path`.
fn check_file(path: &Path, text: &str, lines_to_check: usize) -> std::io::Result<bool> {
    let f = std::fs::File::open(path)?;
    let reader = BufReader::new(f);
    for line in reader.lines().take(lines_to_check) {
        if line?.contains(text) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn glob_match(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.splitn(2, '*').collect();
    match parts.as_slice() {
        [only] => name == *only || name.ends_with(&format!("/{only}")),
        [prefix, suffix] => {
            let anchor_start = prefix.is_empty() || name.starts_with(prefix) || {
                name.contains('/') && {
                    let after_slash = name.rfind('/').map(|i| &name[i + 1..]).unwrap_or(name);
                    prefix.is_empty() || after_slash.starts_with(prefix)
                }
            };
            anchor_start && name.ends_with(suffix)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_match_extension() {
        assert!(glob_match("*.java", "Foo.java"));
        assert!(glob_match("*.java", "src/main/Foo.java"));
        assert!(!glob_match("*.java", "Foo.kt"));
    }

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("Makefile", "Makefile"));
        assert!(glob_match("Makefile", "src/Makefile"));
        assert!(!glob_match("Makefile", "GNUmakefile"));
    }

    #[test]
    fn check_file_finds_header_in_first_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Foo.java");
        std::fs::write(&path, "// Copyright 2024 Acme\npublic class Foo {}\n").unwrap();
        assert!(check_file(&path, "Copyright", 5).unwrap());
    }

    #[test]
    fn check_file_missing_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Foo.java");
        std::fs::write(&path, "public class Foo {}\n").unwrap();
        assert!(!check_file(&path, "Copyright", 5).unwrap());
    }

    #[test]
    fn check_file_header_beyond_line_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Foo.java");
        std::fs::write(
            &path,
            "line1\nline2\nline3\n// Copyright 2024 Acme\npublic class Foo {}\n",
        )
        .unwrap();
        // Header is on line 4; with limit=3 it should not be found.
        assert!(!check_file(&path, "Copyright", 3).unwrap());
        // With limit=5 it should be found.
        assert!(check_file(&path, "Copyright", 5).unwrap());
    }
}
