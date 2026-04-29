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
#[cfg(test)]
pub(crate) use obsolete::latest_registry_tool_migration_target_version;
pub use obsolete::{
    find_obsolete_key, find_unsupported_key, obsolete_keys, obsolete_keys_after, unsupported_keys,
};
pub use resolve::binary_on_path;
pub use types::{
    AdaptiveRelevanceContext, Category, Check, CheckKind, ConfigBase, ConfigFile, ConfigMatch,
    EditorconfigDirectiveStyle, EditorconfigLineLengthPolicy, FixBehavior, InitHookContext,
    LinterConfig, MissingComponentHint, NonverboseFailureOutputHook, RunPolicy, Scope, SpecialKind,
    StatusContext, WorkflowSetup,
};

/// Returns the explicit set of flint-managed tool keys that belong under the
/// `# Linters` header in `mise.toml`.
///
/// This is intentionally invite-only: runtime, SDK, repo-specific, and unknown
/// tools stay above the header unless flint explicitly manages them. Unsupported
/// legacy lint tools are included so existing configs still group lint-related
/// entries together before `flint init` removes or replaces them.
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
    keys.extend(obsolete::obsolete_keys().into_iter().map(|(old, _)| old));
    keys.extend(obsolete::unsupported_keys().into_iter().map(|(old, _)| old));
    keys.insert("github:grafana/flint");
    keys.insert("cargo:https://github.com/grafana/flint");
    keys.insert("cargo:https://github.com/grafana/flint.git");
    keys
}

#[cfg(test)]
mod tests;
