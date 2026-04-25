use std::collections::HashMap;

pub fn obsolete_keys() -> Vec<(&'static str, &'static str)> {
    let mut keys = crate::setup::obsolete_keys();
    keys.extend(registry_tool_key_migrations());
    keys
}

pub fn unsupported_keys() -> Vec<(&'static str, &'static str)> {
    crate::setup::unsupported_keys()
}

pub fn obsolete_keys_after(version: u32) -> Vec<(&'static str, &'static str)> {
    let mut keys = crate::setup::obsolete_keys_after(version);
    keys.extend(registry_tool_key_migrations_after(version));
    keys
}

pub fn find_obsolete_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    obsolete_keys()
        .into_iter()
        .find(|(old, _)| obsolete_key_present(mise_tools, old))
}

pub fn find_unsupported_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    crate::setup::find_unsupported_key(mise_tools)
}

#[cfg(test)]
pub(crate) fn latest_registry_tool_migration_target_version() -> Option<u32> {
    crate::registry::builtin()
        .into_iter()
        .flat_map(|check| check.tool_key_migrations.into_iter())
        .map(|migration| migration.after_setup_version + 1)
        .max()
}

fn registry_tool_key_migrations() -> Vec<(&'static str, &'static str)> {
    crate::registry::builtin()
        .into_iter()
        .filter_map(|check| {
            let new_key = check.install_key()?;
            Some(
                check
                    .tool_key_migrations
                    .into_iter()
                    .map(move |migration| (migration.old_key, new_key)),
            )
        })
        .flatten()
        .collect()
}

fn registry_tool_key_migrations_after(version: u32) -> Vec<(&'static str, &'static str)> {
    crate::registry::builtin()
        .into_iter()
        .filter_map(|check| {
            let new_key = check.install_key()?;
            Some(
                check
                    .tool_key_migrations
                    .into_iter()
                    .filter(move |migration| version <= migration.after_setup_version)
                    .map(move |migration| (migration.old_key, new_key)),
            )
        })
        .flatten()
        .collect()
}

fn obsolete_key_present(mise_tools: &HashMap<String, String>, old: &str) -> bool {
    if old == "shellcheck" && mise_tools.contains_key("github:koalaman/shellcheck") {
        return false;
    }
    mise_tools.contains_key(old)
}
