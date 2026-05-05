use std::collections::HashMap;

const UNSUPPORTED_KEYS_TO_SETUP_VERSION_2: &[(&str, &str)] = &[
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

pub fn find_unsupported_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    UNSUPPORTED_KEYS_TO_SETUP_VERSION_2
        .iter()
        .find(|(old, _)| mise_tools.contains_key(*old))
        .copied()
}

pub fn obsolete_keys() -> Vec<(&'static str, &'static str)> {
    vec![]
}

pub fn unsupported_keys() -> Vec<(&'static str, &'static str)> {
    UNSUPPORTED_KEYS_TO_SETUP_VERSION_2.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_migration_keys_are_unique() {
        let mut unsupported_seen = std::collections::HashSet::new();

        for (old, _) in UNSUPPORTED_KEYS_TO_SETUP_VERSION_2 {
            assert!(
                unsupported_seen.insert(*old),
                "duplicate unsupported setup migration key: {old}"
            );
        }
    }

    #[test]
    fn unsupported_tombstones_are_explicit() {
        let unsupported = unsupported_keys();
        assert!(
            unsupported
                .iter()
                .any(|(old, _)| *old == "npm:markdownlint-cli2")
        );
        assert!(unsupported.iter().any(|(old, _)| *old == "npm:prettier"));
    }
}
