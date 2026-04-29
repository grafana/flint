use anyhow::Result;
use std::path::Path;

use crate::registry::{InitHookContext, StaticLinter};

pub(crate) static LINTER: StaticLinter = StaticLinter::with_init_hook("yamllint", init);

pub(crate) fn init(ctx: &dyn InitHookContext) -> Result<bool> {
    generate_config(ctx.config_dir(), ctx.line_length())
}

pub(crate) fn generate_config(config_dir: &Path, line_length: u16) -> Result<bool> {
    let target = config_dir.join(".yamllint.yml");
    if target.exists() {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = [
        "extends: relaxed",
        "",
        "rules:",
        "  document-start: disable",
        "  line-length:",
        &format!("    max: {line_length}"),
        "  indentation: enable",
        "",
    ]
    .join("\n");
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}
