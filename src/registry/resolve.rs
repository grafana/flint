use std::collections::HashMap;

use super::types::Check;

/// Returns the binary name to use for this check given the active mise tools.
/// When `versioned_bin_fmt` is set, the version from mise.toml is substituted
/// into the format string (e.g. `"shfmt_{version}"` + `"v3.12.0"` → `"shfmt_v3.12.0"`).
/// This is needed for shfmt because mise's `github:` backend preserves the version
/// suffix in the installed binary name. The backend's binary-name cleaning logic matches
/// binaries against the repo name (e.g. `"mvdan/sh"`), so it cannot map `"shfmt"` →
/// `"mvdan/sh"` and leaves the name as `"shfmt_v3.12.0"` rather than stripping it.
///
/// When the exact constructed name is not found on PATH (e.g. after a version bump
/// where the declared version doesn't yet match the installed binary), the function
/// falls back to scanning PATH for any binary whose name starts with the prefix before
/// `{version}` in the format string (e.g. prefix `"shfmt_"` matches `"shfmt_v3.13.1"`).
/// This avoids needing to update fixture versions on every Renovate bump.
pub fn resolve_bin_name(check: &Check, mise_tools: &HashMap<String, String>) -> String {
    if let Some(fmt) = check.versioned_bin_fmt {
        let key = check.mise_tool_name.unwrap_or(check.bin_name);
        if let Some(version) = mise_tools.get(key) {
            let exact = fmt.replace("{version}", version);
            let path_var = std::env::var("PATH").unwrap_or_default();
            if binary_on_path_var(&exact, &path_var) {
                return exact;
            }
            // Exact name not found — scan PATH for any binary starting with the
            // prefix before `{version}` in the format string.
            if let Some(prefix) = fmt.split_once("{version}").map(|(p, _)| p)
                && let Some(found) = find_bin_with_prefix(prefix, &path_var)
            {
                return found;
            }
            return exact;
        }
    }
    check.bin_name.to_string()
}

/// Scans each directory in `path_var` for the first file whose name starts with
/// `prefix`. Returns the file name (not the full path) of the first match found.
fn find_bin_with_prefix(prefix: &str, path_var: &str) -> Option<String> {
    for dir in std::env::split_paths(path_var) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(prefix) && entry.path().is_file() {
                return Some(name_str.into_owned());
            }
        }
    }
    None
}

/// Returns true if `bin_name` exists as a file in any directory in `path_var`
/// (a `:`-separated PATH string). Accepts the PATH string as a parameter so
/// callers can substitute a test-controlled path without mutating env vars.
pub fn binary_on_path_var(bin_name: &str, path_var: &str) -> bool {
    std::env::split_paths(path_var).any(|dir| dir.join(bin_name).is_file())
}

/// Returns true if `bin_name` is found in the current `PATH`.
pub fn binary_on_path(bin_name: &str) -> bool {
    binary_on_path_var(bin_name, &std::env::var("PATH").unwrap_or_default())
}
