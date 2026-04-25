use std::collections::HashMap;

// Name only durable setup boundaries. Routine migration targets can stay numeric
// in SETUP_MIGRATIONS unless they become a baseline that call sites need to
// reference directly. LATEST_SUPPORTED_SETUP_VERSION is intentionally the only
// moving constant.
pub const V1_BOOTSTRAP_SETUP_VERSION: u32 = 0;
pub const V2_BASELINE_SETUP_VERSION: u32 = 1;
pub const LATEST_SUPPORTED_SETUP_VERSION: u32 = 2;

pub struct SetupMigration {
    pub target_version: u32,
    pub obsolete_keys: &'static [(&'static str, &'static str)],
    pub unsupported_keys: &'static [(&'static str, &'static str)],
}

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
        "replace with rumdl and yaml-lint, then remove prettier from the lint toolchain",
    ),
];

pub const SETUP_MIGRATIONS: &[SetupMigration] = &[
    SetupMigration {
        target_version: V2_BASELINE_SETUP_VERSION,
        obsolete_keys: &[],
        unsupported_keys: &[],
    },
    SetupMigration {
        target_version: LATEST_SUPPORTED_SETUP_VERSION,
        obsolete_keys: &[],
        unsupported_keys: UNSUPPORTED_KEYS_TO_SETUP_VERSION_2,
    },
];

pub fn find_unsupported_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    SETUP_MIGRATIONS
        .iter()
        .flat_map(|migration| migration.unsupported_keys.iter())
        .find(|(old, _)| mise_tools.contains_key(*old))
        .copied()
}

pub fn obsolete_keys() -> Vec<(&'static str, &'static str)> {
    SETUP_MIGRATIONS
        .iter()
        .flat_map(|migration| migration.obsolete_keys.iter().copied())
        .collect()
}

pub fn unsupported_keys() -> Vec<(&'static str, &'static str)> {
    SETUP_MIGRATIONS
        .iter()
        .flat_map(|migration| migration.unsupported_keys.iter().copied())
        .collect()
}

pub fn obsolete_keys_after(version: u32) -> Vec<(&'static str, &'static str)> {
    SETUP_MIGRATIONS
        .iter()
        .filter(|migration| migration.target_version > version)
        .flat_map(|migration| migration.obsolete_keys.iter().copied())
        .collect()
}

pub fn unsupported_keys_after(version: u32) -> Vec<(&'static str, &'static str)> {
    SETUP_MIGRATIONS
        .iter()
        .filter(|migration| migration.target_version > version)
        .flat_map(|migration| migration.unsupported_keys.iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_supported_setup_version_matches_latest_migration_target() {
        let latest_setup_migration = SETUP_MIGRATIONS
            .iter()
            .map(|migration| migration.target_version)
            .max()
            .unwrap_or(0);
        let latest_registry_migration =
            crate::registry::latest_registry_tool_migration_target_version().unwrap_or(0);
        let latest = latest_setup_migration.max(latest_registry_migration);
        assert_eq!(
            LATEST_SUPPORTED_SETUP_VERSION, latest,
            "LATEST_SUPPORTED_SETUP_VERSION must match the latest setup migration target version"
        );
    }

    #[test]
    fn setup_migration_versions_are_strictly_increasing() {
        let mut previous = V1_BOOTSTRAP_SETUP_VERSION;
        for migration in SETUP_MIGRATIONS {
            assert!(
                migration.target_version > previous,
                "setup migration versions must be strictly increasing"
            );
            previous = migration.target_version;
        }
    }

    #[test]
    fn setup_migration_keys_are_unique() {
        let mut obsolete_seen = std::collections::HashSet::new();
        let mut unsupported_seen = std::collections::HashSet::new();

        for migration in SETUP_MIGRATIONS {
            for (old, _) in migration.obsolete_keys {
                assert!(
                    obsolete_seen.insert(*old),
                    "duplicate obsolete setup migration key: {old}"
                );
            }
            for (old, _) in migration.unsupported_keys {
                assert!(
                    unsupported_seen.insert(*old),
                    "duplicate unsupported setup migration key: {old}"
                );
            }
        }
    }

    #[test]
    fn v2_baseline_tombstones_are_explicit() {
        let obsolete = obsolete_keys_after(V2_BASELINE_SETUP_VERSION);
        let unsupported = unsupported_keys_after(V2_BASELINE_SETUP_VERSION);

        assert!(
            obsolete.is_empty(),
            "live tool-key migrations should live in the registry"
        );
        assert!(
            unsupported
                .iter()
                .any(|(old, _)| *old == "npm:markdownlint-cli2")
        );
        assert!(unsupported.iter().any(|(old, _)| *old == "npm:prettier"));
        assert!(obsolete_keys_after(LATEST_SUPPORTED_SETUP_VERSION).is_empty());
        assert!(unsupported_keys_after(LATEST_SUPPORTED_SETUP_VERSION).is_empty());
    }
}
