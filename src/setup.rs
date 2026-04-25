use std::collections::HashMap;

pub const V1_SETUP_VERSION: u32 = 0;
pub const DEPLOYED_SETUP_VERSION: u32 = 1;
pub const CURRENT_SETUP_VERSION: u32 = 2;

pub struct SetupMigration {
    pub target_version: u32,
    pub obsolete_keys: &'static [(&'static str, &'static str)],
    pub unsupported_keys: &'static [(&'static str, &'static str)],
}

const OBSOLETE_KEYS_TO_DEPLOYED: &[(&str, &str)] = &[
    // markdownlint-cli was superseded by markdownlint-cli2 before the deployed
    // v2 baseline. Keep this migration so old v1 repos can converge.
    ("npm:markdownlint-cli", "npm:markdownlint-cli2"),
    // ubi: was deprecated in mise; the github: backend is the modern replacement.
    // Repos that adopted flint before this change may still have ubi: keys.
    (
        "ubi:google/google-java-format",
        "github:google/google-java-format",
    ),
    ("ubi:pinterest/ktlint", "github:pinterest/ktlint"),
    // github:mvdan/sh is superseded by bare shfmt; mise resolves it via aqua:mvdan/sh,
    // and the aqua registry now ships Windows support for shfmt.
    ("github:mvdan/sh", "shfmt"),
];

const OBSOLETE_KEYS_TO_NEXT: &[(&str, &str)] = &[
    ("github:pinterest/ktlint", "ktlint"),
    // ryl is available from aqua-registry, but current mise releases still require
    // the explicit aqua-prefixed key instead of exposing a bare `ryl` tool.
    ("cargo:yaml-lint", "aqua:owenlamont/ryl"),
    ("github:owenlamont/ryl", "aqua:owenlamont/ryl"),
    // Ruff is available as a bare aqua-backed tool key.
    ("pipx:ruff", "ruff"),
    ("github:astral-sh/ruff", "ruff"),
    ("github:tamasfe/taplo", "taplo"),
    // Bare shellcheck currently resolves through aqua in mise, but that path
    // failed Windows CI. Use the GitHub backend until the aqua entry is fixed.
    ("shellcheck", "github:koalaman/shellcheck"),
    // npm-installed biome is superseded by the standalone biome binary.
    ("npm:@biomejs/biome", "biome"),
    // xmloxide now publishes GitHub releases consumable via mise's github: backend.
    ("cargo:xmloxide", "github:jonwiggins/xmloxide"),
];

const UNSUPPORTED_KEYS_TO_NEXT: &[(&str, &str)] = &[
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
        target_version: DEPLOYED_SETUP_VERSION,
        obsolete_keys: OBSOLETE_KEYS_TO_DEPLOYED,
        unsupported_keys: &[],
    },
    SetupMigration {
        target_version: CURRENT_SETUP_VERSION,
        obsolete_keys: OBSOLETE_KEYS_TO_NEXT,
        unsupported_keys: UNSUPPORTED_KEYS_TO_NEXT,
    },
];

pub fn find_obsolete_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    SETUP_MIGRATIONS
        .iter()
        .flat_map(|migration| migration.obsolete_keys.iter())
        .find(|(old, _)| obsolete_key_present(mise_tools, old))
        .copied()
}

fn obsolete_key_present(mise_tools: &HashMap<String, String>, old: &str) -> bool {
    if old == "shellcheck" && mise_tools.contains_key("github:koalaman/shellcheck") {
        return false;
    }
    mise_tools.contains_key(old)
}

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
    fn current_setup_version_matches_latest_migration() {
        let latest = SETUP_MIGRATIONS
            .iter()
            .map(|migration| migration.target_version)
            .max()
            .unwrap_or(0);
        assert_eq!(
            CURRENT_SETUP_VERSION, latest,
            "CURRENT_SETUP_VERSION must match the latest setup migration target version"
        );
    }

    #[test]
    fn setup_migration_versions_are_strictly_increasing() {
        let mut previous = V1_SETUP_VERSION;
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
    fn deployed_baseline_migrations_are_explicit() {
        let obsolete = obsolete_keys_after(DEPLOYED_SETUP_VERSION);
        let unsupported = unsupported_keys_after(DEPLOYED_SETUP_VERSION);

        assert!(obsolete.contains(&("pipx:ruff", "ruff")));
        assert!(obsolete.contains(&("shellcheck", "github:koalaman/shellcheck")));
        assert!(
            unsupported
                .iter()
                .any(|(old, _)| *old == "npm:markdownlint-cli2")
        );
        assert!(unsupported.iter().any(|(old, _)| *old == "npm:prettier"));
        assert!(obsolete_keys_after(CURRENT_SETUP_VERSION).is_empty());
        assert!(unsupported_keys_after(CURRENT_SETUP_VERSION).is_empty());
    }

    #[test]
    fn shellcheck_alias_does_not_make_github_backend_obsolete() {
        let tools = HashMap::from([
            (
                "github:koalaman/shellcheck".to_string(),
                "0.11.0".to_string(),
            ),
            ("shellcheck".to_string(), "0.11.0".to_string()),
        ]);

        assert_eq!(find_obsolete_key(&tools), None);
    }
}
