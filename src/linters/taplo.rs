use anyhow::Result;
use std::path::Path;

use crate::registry::InitHookContext;

pub(crate) fn init(ctx: &dyn InitHookContext) -> Result<bool> {
    generate_config(ctx.config_dir(), ctx.line_length())
}

pub(crate) fn generate_config(config_dir: &Path, line_length: u16) -> Result<bool> {
    const SUPPORTED_CONFIG_NAMES: &[&str] = &[".taplo.toml"];
    const LEGACY_CONFIG_NAMES: &[&str] = &["taplo.toml"];
    if SUPPORTED_CONFIG_NAMES
        .iter()
        .map(|name| config_dir.join(name))
        .any(|path| path.exists())
        || LEGACY_CONFIG_NAMES
            .iter()
            .map(|name| config_dir.join(name))
            .any(|path| path.exists())
    {
        return Ok(false);
    }
    let target = config_dir.join(".taplo.toml");
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = [
        "[formatting]".to_string(),
        format!("column_width = {line_length}"),
        "indent_string = \"  \"".to_string(),
    ]
    .join("\n")
        + "\n";
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

pub(crate) fn normalize_nonverbose_failure_output(
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
