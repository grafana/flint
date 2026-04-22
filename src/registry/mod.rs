mod checks;
mod mise;
mod obsolete;
mod resolve;
mod types;

pub use checks::builtin;
pub use mise::{check_active, read_mise_tools};
pub use obsolete::{OBSOLETE_KEYS, find_obsolete_key, find_unsupported_key};
pub use resolve::binary_on_path;
pub use types::{Category, Check, CheckKind, FixBehavior, RunPolicy, Scope, SpecialKind};

/// Returns the set of `mise.toml` tool keys that name language runtimes/SDKs
/// (e.g. `rust`, `go`, `dotnet`). Derived from registry checks marked
/// `.toolchain()`.
///
/// `flint init` uses this set to keep runtime keys above the `# Linters`
/// header in `mise.toml`.
pub fn toolchain_keys() -> std::collections::HashSet<&'static str> {
    builtin()
        .into_iter()
        .filter(|c| c.is_toolchain())
        .filter_map(|c| c.mise_tool_name)
        .collect()
}

#[cfg(test)]
mod tests;
