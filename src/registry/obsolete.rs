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
    // github:mvdan/sh is superseded by bare shfmt; mise resolves it via aqua:mvdan/sh,
    // and the aqua registry now ships Windows support for shfmt.
    ("github:mvdan/sh", "shfmt"),
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
