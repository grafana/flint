use anyhow::Result;
use std::path::Path;

use crate::registry::{CheckTypeDef, InitHookContext};

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::with_init_hook("biome", init);

pub(crate) fn init(ctx: &dyn InitHookContext) -> Result<bool> {
    generate_config(ctx.project_root())
}

pub(crate) fn generate_config(project_root: &Path) -> Result<bool> {
    let target = project_root.join("biome.jsonc");
    if target.exists() {
        return Ok(false);
    }
    let legacy = project_root.join("biome.json");
    if legacy.exists() {
        std::fs::rename(&legacy, &target)?;
        println!("  moved {} -> {}", legacy.display(), target.display());
        return Ok(true);
    }
    let content = [
        "{",
        "  \"formatter\": {",
        "    \"indentStyle\": \"space\",",
        "    \"indentWidth\": 2",
        "  }",
        "}",
        "",
    ]
    .join("\n");
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}
