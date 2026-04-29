use anyhow::Result;

use crate::registry::InitHookContext;

use super::super::renovate::{configure_renovate_deps_config, patch_renovate_preset};

pub(crate) fn run(ctx: &dyn InitHookContext) -> Result<bool> {
    let toml_path = ctx.config_dir().join("flint.toml");
    let config_changed = if let Some(managers) = ctx.renovate_exclude_managers()
        && !managers.is_empty()
    {
        configure_renovate_deps_config(&toml_path, Some(managers))?
    } else if ctx.flint_toml_generated() {
        configure_renovate_deps_config(&toml_path, None)?
    } else {
        false
    };
    let preset_changed = patch_renovate_preset(ctx.project_root())?;
    Ok(config_changed || preset_changed)
}
