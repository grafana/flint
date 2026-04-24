mod checks;
mod mise;
mod obsolete;
mod resolve;
mod types;

pub use checks::builtin;
pub use mise::{
    check_active, flint_version_changed, read_mise_tools, read_mise_tools_at_ref,
    tool_version_changed,
};
pub use obsolete::{OBSOLETE_KEYS, find_obsolete_key, find_unsupported_key};
pub use resolve::binary_on_path;
pub use types::{
    Category, Check, CheckKind, ConfigBase, ConfigFile, ConfigMatch, FixBehavior, LinterConfig,
    RunPolicy, Scope, SpecialKind,
};

/// Returns the set of `mise.toml` tool keys that belong under the `# Linters`
/// header. Runtime, SDK, and unrelated project tools stay above the header.
///
/// Includes unsupported legacy lint tools so existing configs still group
/// lint-related entries together before `flint init` removes or replaces them.
pub fn linter_keys() -> std::collections::HashSet<&'static str> {
    let mut keys: std::collections::HashSet<&'static str> = std::collections::HashSet::new();
    for check in builtin()
        .into_iter()
        .filter(|c| c.uses_binary() && !c.is_toolchain() && !c.activate_unconditionally)
    {
        keys.insert(check.bin_name);
        if let Some(tool) = check.mise_tool_name {
            keys.insert(tool);
        }
    }
    keys.extend(OBSOLETE_KEYS.iter().map(|(old, _)| *old));
    keys.extend(obsolete::UNSUPPORTED_KEYS.iter().map(|(old, _)| *old));
    keys.insert("github:grafana/flint");
    keys
}

#[cfg(test)]
mod tests;
