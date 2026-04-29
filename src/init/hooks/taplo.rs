use anyhow::Result;
use std::path::Path;

use crate::registry::InitHookContext;

pub(crate) fn run(ctx: &dyn InitHookContext) -> Result<bool> {
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
