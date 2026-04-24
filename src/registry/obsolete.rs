use std::collections::HashMap;

/// Mise tool keys that are no longer supported by flint and should be removed
/// during `flint init`. Each entry is `(old_key, replacement_key)` where
/// `replacement_key` is the modern equivalent that the registry now uses.
pub const OBSOLETE_KEYS: &[(&str, &str)] = &[
    // ubi: was deprecated in mise; the github: backend is the modern replacement.
    // Repos that adopted flint before this change may still have ubi: keys.
    (
        "ubi:google/google-java-format",
        "github:google/google-java-format",
    ),
    ("ubi:pinterest/ktlint", "github:pinterest/ktlint"),
    // ryl ships standalone GitHub release binaries, so we no longer need the
    // cargo backend for yaml-lint.
    ("cargo:yaml-lint", "github:owenlamont/ryl"),
    // Ruff ships standalone GitHub release binaries, so we no longer need the
    // pipx backend or a Python runtime just to install it.
    ("pipx:ruff", "github:astral-sh/ruff"),
    // github:mvdan/sh is superseded by bare shfmt; mise resolves it via aqua:mvdan/sh,
    // and the aqua registry now ships Windows support for shfmt.
    ("github:mvdan/sh", "shfmt"),
    // npm-installed biome is superseded by the standalone biome binary.
    ("npm:@biomejs/biome", "biome"),
];

/// Mise tool keys that flint no longer supports and cannot auto-rewrite 1:1.
/// These require a docs/config migration rather than a backend swap.
pub const UNSUPPORTED_KEYS: &[(&str, &str)] = &[
    (
        "npm:markdownlint-cli2",
        "replace with rumdl and remove markdownlint-era config",
    ),
    (
        "npm:prettier",
        "replace with rumdl and yaml-lint, then remove prettier from the lint toolchain",
    ),
];

/// Checks whether any obsolete tool keys are present in `mise_tools`.
/// Returns the first violation found as `(obsolete_key, replacement_key)`.
pub fn find_obsolete_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    OBSOLETE_KEYS
        .iter()
        .find(|(old, _)| mise_tools.contains_key(*old))
        .copied()
}

/// Checks whether any unsupported legacy tool keys are present in `mise_tools`.
/// Returns the first violation found as `(unsupported_key, migration_hint)`.
pub fn find_unsupported_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    UNSUPPORTED_KEYS
        .iter()
        .find(|(old, _)| mise_tools.contains_key(*old))
        .copied()
}
