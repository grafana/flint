use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::config::LicenseHeaderConfig;
use crate::files::match_files;
use crate::linters::LinterOutput;
use crate::registry::{
    PreparedSpecialCheck, SpecialPrepareContext, SpecialRunContext, SpecialRunFuture, StaticLinter,
    StaticSpecialLinter, StatusContext,
};

pub(crate) static LINTER: StaticLinter =
    StaticLinter::special("license-header", StaticSpecialLinter::new(false, prepare));

#[derive(Debug)]
struct PreparedLicenseHeader {
    name: String,
    cfg: LicenseHeaderConfig,
    files: Vec<PathBuf>,
}

fn prepare(ctx: SpecialPrepareContext<'_>) -> Option<Box<dyn PreparedSpecialCheck>> {
    if ctx.cfg.checks.license_header.text.is_empty() {
        return None;
    }
    let patterns: Vec<&str> = ctx
        .cfg
        .checks
        .license_header
        .patterns
        .iter()
        .map(String::as_str)
        .collect();
    let files: Vec<PathBuf> = match_files(&ctx.file_list.files, &patterns, &[], ctx.project_root)
        .into_iter()
        .cloned()
        .collect();
    if files.is_empty() {
        return None;
    }
    Some(Box::new(PreparedLicenseHeader {
        name: ctx.name.to_string(),
        cfg: ctx.cfg.checks.license_header.clone(),
        files,
    }))
}

impl PreparedSpecialCheck for PreparedLicenseHeader {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(self: Box<Self>, ctx: SpecialRunContext) -> SpecialRunFuture {
        Box::pin(async move {
            crate::linters::license_header::run(&self.cfg, &ctx.project_root, &self.files).await
        })
    }
}

/// Checks that each file contains `cfg.text` within the first `cfg.lines_to_check` lines.
/// Files are pre-filtered by pattern in the runner; this function checks all of them.
pub async fn run(
    cfg: &LicenseHeaderConfig,
    project_root: &Path,
    files: &[PathBuf],
) -> LinterOutput {
    let mut all_ok = true;
    let mut stderr = Vec::new();

    for file in files {
        let rel = file.strip_prefix(project_root).unwrap_or(file);
        let rel_str = rel.to_string_lossy();

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

pub(crate) fn status(ctx: &dyn StatusContext) -> Option<&'static str> {
    ctx.config()
        .checks
        .license_header
        .text
        .is_empty()
        .then_some("not configured")
}

/// Returns `true` if `text` appears anywhere within the first `lines_to_check` lines of `path`.
/// `text` may be multi-line; the file head is joined with `\n` before the substring search.
fn check_file(path: &Path, text: &str, lines_to_check: usize) -> std::io::Result<bool> {
    let f = std::fs::File::open(path)?;
    let reader = BufReader::new(f);
    let head = reader
        .lines()
        .take(lines_to_check)
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    Ok(head.contains(text))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn check_file_multiline_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Foo.java");
        std::fs::write(
            &path,
            "/*\n * Copyright Acme\n * SPDX-License-Identifier: Apache-2.0\n */\npublic class Foo {}\n",
        )
        .unwrap();
        let header = "/*\n * Copyright Acme\n * SPDX-License-Identifier: Apache-2.0\n */";
        assert!(check_file(&path, header, 5).unwrap());
        // Partial match still works (single-line substring within the joined head)
        assert!(check_file(&path, "SPDX-License-Identifier: Apache-2.0", 5).unwrap());
        // Text that spans more lines than the limit is not found
        assert!(!check_file(&path, header, 2).unwrap());
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
