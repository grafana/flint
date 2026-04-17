use std::collections::HashMap;

/// Mise tool keys that are no longer supported by flint and should be removed
/// during `flint init`. Each entry is `(old_key, replacement_key)` where
/// `replacement_key` is the modern equivalent that the registry now uses.
pub const OBSOLETE_KEYS: &[(&str, &str)] = &[
    // markdownlint-cli was superseded by markdownlint-cli2 (actively maintained,
    // faster, supports the same config files). flint only supports the cli2 variant.
    ("npm:markdownlint-cli", "npm:markdownlint-cli2"),
    // ubi: was deprecated in mise; the github: backend is the modern replacement.
    // Repos that adopted flint before this change may still have ubi: keys.
    (
        "ubi:google/google-java-format",
        "github:google/google-java-format",
    ),
    ("ubi:pinterest/ktlint", "github:pinterest/ktlint"),
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
