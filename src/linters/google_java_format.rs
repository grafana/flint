use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::config::{GoogleJavaFormatConfig, OffOnMarkerConfig};
use crate::files::match_files;
use crate::linters::{LinterOutput, spawn_command};
use crate::regions::{RegionError, RegionSpan, find_region_spans};
use crate::registry::{
    CheckTypeDef, NativeCheckDef, NativePrepareContext, NativeRunContext, NativeRunFuture,
    PreparedNativeCheck,
};

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::native(
    "google-java-format",
    NativeCheckDef::with_bin("google-java-format", prepare).with_fix(),
);

#[derive(Debug)]
struct PreparedGoogleJavaFormat {
    name: String,
    cfg: GoogleJavaFormatConfig,
    files: Vec<PathBuf>,
}

fn prepare(ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>> {
    let configured_patterns: Vec<&str> = ctx
        .cfg
        .checks
        .google_java_format
        .patterns
        .iter()
        .map(String::as_str)
        .collect();
    let patterns = if configured_patterns.is_empty() {
        vec!["*.java"]
    } else {
        configured_patterns
    };
    let excludes: Vec<&str> = ctx
        .cfg
        .checks
        .google_java_format
        .exclude
        .iter()
        .map(String::as_str)
        .collect();
    let files: Vec<PathBuf> =
        match_files(&ctx.file_list.files, &patterns, &excludes, ctx.project_root)
            .into_iter()
            .cloned()
            .collect();

    if files.is_empty() {
        return None;
    }

    Some(Box::new(PreparedGoogleJavaFormat {
        name: ctx.name.to_string(),
        cfg: ctx.cfg.checks.google_java_format.clone(),
        files,
    }))
}

impl PreparedNativeCheck for PreparedGoogleJavaFormat {
    fn name(&self) -> &str {
        &self.name
    }

    fn tracked_files(&self) -> &[PathBuf] {
        &self.files
    }

    fn run(self: Box<Self>, ctx: NativeRunContext) -> NativeRunFuture {
        Box::pin(async move { run(&self.cfg, &ctx.project_root, &self.files, ctx.fix).await })
    }
}

pub(crate) async fn run(
    cfg: &GoogleJavaFormatConfig,
    project_root: &Path,
    files: &[PathBuf],
    fix: bool,
) -> LinterOutput {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut all_ok = true;
    let mut batch = Vec::new();

    for file in files {
        if has_configured_marker(file, &cfg.off_on_markers) {
            if let Err(error) =
                run_marked_file(cfg, project_root, file, fix, &mut stdout, &mut stderr).await
            {
                all_ok = false;
                if error.to_string() != "formatting changes required" {
                    append_error(&mut stderr, project_root, file, &error);
                }
            }
        } else {
            batch.push(file);
        }
    }

    for chunk in chunks(&batch) {
        match run_batch(cfg, project_root, chunk, fix).await {
            Ok(output) => {
                if !output.status.success() {
                    all_ok = false;
                }
                stdout.extend_from_slice(&output.stdout);
                stderr.extend_from_slice(&output.stderr);
            }
            Err(error) => {
                all_ok = false;
                stderr
                    .extend_from_slice(format!("flint: google-java-format: {error}\n").as_bytes());
            }
        }
    }

    LinterOutput {
        ok: all_ok,
        stdout,
        stderr,
        setup_outcome: None,
    }
}

fn build_args(cfg: &GoogleJavaFormatConfig, fix: bool) -> Vec<String> {
    let mut args = vec!["google-java-format".to_string()];
    if cfg.aosp {
        args.push("--aosp".to_string());
    }
    if cfg.skip_reflowing_long_strings {
        args.push("--skip-reflowing-long-strings".to_string());
    }
    if cfg.skip_sorting_imports {
        args.push("--skip-sorting-imports".to_string());
    }
    if cfg.skip_removing_unused_imports {
        args.push("--skip-removing-unused-imports".to_string());
    }
    if cfg.skip_javadoc_formatting {
        args.push("--skip-javadoc-formatting".to_string());
    }
    if fix {
        args.push("-i".to_string());
    } else {
        args.extend(["--dry-run".to_string(), "--set-exit-if-changed".to_string()]);
    }
    args
}

async fn run_batch(
    cfg: &GoogleJavaFormatConfig,
    project_root: &Path,
    files: &[&PathBuf],
    fix: bool,
) -> std::io::Result<std::process::Output> {
    let mut args = build_args(cfg, fix);
    args.extend(files.iter().map(|file| file.to_string_lossy().into_owned()));
    spawn_command(&args, false)
        .current_dir(project_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
}

async fn run_marked_file(
    cfg: &GoogleJavaFormatConfig,
    project_root: &Path,
    file: &Path,
    fix: bool,
    stdout: &mut Vec<u8>,
    stderr: &mut Vec<u8>,
) -> std::io::Result<()> {
    let original = std::fs::read_to_string(file)?;
    let tempdir = tempfile::tempdir()?;
    let temp_file = tempdir.path().join(
        file.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("source.java"),
    );
    std::fs::write(&temp_file, &original)?;

    let mut args = build_args(cfg, true);
    args.push(temp_file.to_string_lossy().into_owned());
    let output = spawn_command(&args, false)
        .current_dir(project_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    stdout.extend_from_slice(&output.stdout);
    stderr.extend_from_slice(&output.stderr);
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "google-java-format exited with status {}",
            output.status.code().unwrap_or(-1)
        )));
    }

    let formatted = std::fs::read_to_string(&temp_file)?;
    let restored = restore_marked_regions(&original, &formatted, &cfg.off_on_markers)
        .map_err(std::io::Error::other)?;
    if restored == original {
        return Ok(());
    }
    if fix {
        std::fs::write(file, restored)?;
    } else {
        let rel = file.strip_prefix(project_root).unwrap_or(file);
        stderr.extend_from_slice(format!("{}\n", rel.to_string_lossy()).as_bytes());
        return Err(std::io::Error::other("formatting changes required"));
    }
    Ok(())
}

fn has_configured_marker(file: &Path, markers: &[OffOnMarkerConfig]) -> bool {
    if markers.is_empty() {
        return false;
    }
    let Ok(content) = std::fs::read_to_string(file) else {
        return false;
    };
    markers.iter().any(|marker| content.contains(&marker.off))
}

fn restore_marked_regions(
    original: &str,
    formatted: &str,
    markers: &[OffOnMarkerConfig],
) -> Result<String, String> {
    let original_lines: Vec<&str> = original.lines().collect();
    let mut formatted_lines: Vec<String> = formatted.lines().map(ToString::to_string).collect();

    let original_regions = find_marker_spans(&original_lines, markers)?;
    let mut formatted_regions = find_marker_spans(
        &formatted_lines
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        markers,
    )?;
    if original_regions.len() != formatted_regions.len() {
        return Err(format!(
            "formatter-off markers changed from {} regions to {}",
            original_regions.len(),
            formatted_regions.len()
        ));
    }

    let mut original_regions = original_regions;
    original_regions.sort_by_key(|region| (region.marker_index, region.start_line));
    formatted_regions.sort_by_key(|region| (region.marker_index, region.start_line));
    for (original_region, formatted_region) in original_regions.iter().zip(&formatted_regions) {
        if original_region.marker_index != formatted_region.marker_index {
            return Err("formatter-off marker pairs changed order".to_string());
        }
    }

    // Replace from the bottom up so earlier line indexes remain valid after
    // restoring a region whose contents have a different number of lines.
    let mut replacements: Vec<_> = original_regions.iter().zip(&formatted_regions).collect();
    replacements.sort_by_key(|(_, formatted)| std::cmp::Reverse(formatted.start_line));
    for (original_region, formatted_region) in replacements {
        formatted_lines.splice(
            formatted_region.start_line + 1..formatted_region.end_line,
            original_lines[original_region.start_line + 1..original_region.end_line]
                .iter()
                .map(|line| (*line).to_string()),
        );
    }

    let mut output = formatted_lines.join("\n");
    if formatted.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

fn find_marker_spans(
    lines: &[&str],
    markers: &[OffOnMarkerConfig],
) -> Result<Vec<RegionSpan>, String> {
    find_region_spans(
        lines,
        markers,
        |marker, line| line.contains(&marker.off),
        |marker, line| line.contains(&marker.on),
    )
    .map_err(|error| format_marker_error(error, markers))
}

fn format_marker_error(error: RegionError, markers: &[OffOnMarkerConfig]) -> String {
    match error {
        RegionError::EndWithoutStart { marker_index, .. } => {
            let marker = &markers[marker_index];
            format!(
                "formatter-off marker {:?} has no matching {:?}",
                marker.on, marker.off
            )
        }
        RegionError::StartWithoutEnd { marker_index, .. } => {
            let marker = &markers[marker_index];
            format!(
                "formatter-off marker {:?} has no matching {:?}",
                marker.off, marker.on
            )
        }
    }
}

fn chunks<'a>(files: &'a [&'a PathBuf]) -> Vec<&'a [&'a PathBuf]> {
    // Keep command lines comfortably below Windows' limit. GJF is invoked in
    // a native check so it does not pass through runner.rs's template chunker.
    const MAX_CHARS: usize = 6_000;
    let mut result = Vec::new();
    let mut start = 0;
    let mut length = 0;
    for (index, file) in files.iter().enumerate() {
        let file_length = file.to_string_lossy().len() + 1;
        if index > start && length + file_length > MAX_CHARS {
            result.push(&files[start..index]);
            start = index;
            length = 0;
        }
        length += file_length;
    }
    if start < files.len() {
        result.push(&files[start..]);
    }
    result
}

fn append_error(stderr: &mut Vec<u8>, project_root: &Path, file: &Path, error: &std::io::Error) {
    let rel = file.strip_prefix(project_root).unwrap_or(file);
    stderr.extend_from_slice(format!("{}: {error}\n", rel.to_string_lossy()).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_regions_between_markers() {
        let marker = OffOnMarkerConfig {
            off: "// spotless:off".to_string(),
            on: "// spotless:on".to_string(),
        };
        let original = "class Test {\n// spotless:off\n  int a=  1;\n// spotless:on\n}\n";
        let formatted = "class Test {\n  // spotless:off\n  int a = 1;\n  // spotless:on\n}\n";
        let restored = restore_marked_regions(original, formatted, &[marker]).unwrap();
        assert_eq!(
            restored,
            "class Test {\n  // spotless:off\n  int a=  1;\n  // spotless:on\n}\n"
        );
    }

    #[test]
    fn builds_configured_gjf_flags() {
        let cfg = GoogleJavaFormatConfig {
            aosp: true,
            skip_reflowing_long_strings: true,
            skip_sorting_imports: true,
            skip_removing_unused_imports: true,
            skip_javadoc_formatting: true,
            ..GoogleJavaFormatConfig::default()
        };
        assert_eq!(
            build_args(&cfg, false),
            vec![
                "google-java-format",
                "--aosp",
                "--skip-reflowing-long-strings",
                "--skip-sorting-imports",
                "--skip-removing-unused-imports",
                "--skip-javadoc-formatting",
                "--dry-run",
                "--set-exit-if-changed",
            ]
        );
    }
}
