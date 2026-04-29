use anyhow::Result;
use std::path::Path;

use crate::registry::{CheckTypeDef, InitHookContext};

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::with_init_hook("rustfmt", init);

pub(crate) fn init(ctx: &dyn InitHookContext) -> Result<bool> {
    generate_config(ctx.config_dir(), ctx.line_length())
}

pub(crate) fn generate_config(config_dir: &Path, line_length: u16) -> Result<bool> {
    let target = config_dir.join("rustfmt.toml");
    if target.exists() {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = format!("max_width = {line_length}\n");
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}
