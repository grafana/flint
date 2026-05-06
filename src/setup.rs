use std::collections::HashMap;

const UNSUPPORTED_TOOL_KEYS: &[(&str, &str)] = &[
    (
        "npm:markdownlint-cli",
        "replace with rumdl and remove markdownlint-era config",
    ),
    (
        "npm:markdownlint-cli2",
        "replace with rumdl and remove markdownlint-era config",
    ),
    (
        "npm:prettier",
        "replace with rumdl and ryl, then remove prettier from the lint toolchain",
    ),
];

const OBSOLETE_KEYS: &[(&str, &str)] = &[("github:grafana/flint", "aqua:grafana/flint")];
pub fn find_unsupported_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    UNSUPPORTED_TOOL_KEYS
        .iter()
        .find(|(old, _)| mise_tools.contains_key(*old))
        .copied()
}

pub fn obsolete_keys() -> Vec<(&'static str, &'static str)> {
    OBSOLETE_KEYS.to_vec()
}

pub fn unsupported_keys() -> Vec<(&'static str, &'static str)> {
    UNSUPPORTED_TOOL_KEYS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_migration_keys_are_unique() {
        let mut obsolete_seen = std::collections::HashSet::new();
        let mut unsupported_seen = std::collections::HashSet::new();

        for (old, _) in OBSOLETE_KEYS {
            assert!(
                obsolete_seen.insert(*old),
                "duplicate obsolete setup migration key: {old}"
            );
        }
        for (old, _) in UNSUPPORTED_TOOL_KEYS {
            assert!(
                unsupported_seen.insert(*old),
                "duplicate unsupported setup migration key: {old}"
            );
        }
    }

    #[test]
    fn unsupported_tombstones_are_explicit() {
        let obsolete = obsolete_keys();
        assert!(
            obsolete
                .iter()
                .any(|(old, new)| *old == "github:grafana/flint" && *new == "aqua:grafana/flint")
        );
        let unsupported = unsupported_keys();
        assert!(
            unsupported
                .iter()
                .any(|(old, _)| *old == "npm:markdownlint-cli2")
        );
        assert!(unsupported.iter().any(|(old, _)| *old == "npm:prettier"));
    }
}
