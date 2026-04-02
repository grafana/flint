use std::collections::HashMap;
use std::path::Path;

/// How a check is invoked relative to the file list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Invoked once per matched file: `{FILE}` placeholder.
    File,
    /// Invoked once with all matched files: `{FILES}` placeholder.
    Files,
    /// Invoked once with no file args (e.g. golangci-lint).
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKind {
    Links,
    RenovateDeps,
}

#[derive(Debug, Clone)]
pub enum CheckKind {
    Template {
        check_cmd: &'static str,
        fix_cmd: &'static str,
        scope: Scope,
    },
    Special(SpecialKind),
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    /// Binary name used to invoke the tool.
    pub bin_name: &'static str,
    /// mise.toml tool key to look up for availability. When `None`, falls back to
    /// `bin_name`. Use this when the binary comes from a toolchain entry rather than
    /// its own tool entry (e.g. `cargo-fmt` ships with `rust`).
    pub mise_tool_name: Option<&'static str>,
    /// Semver requirement string (e.g. `">=1.0.0"`). When `None`, any version matches.
    /// When multiple registry entries share a `bin_name`, each must have a `version_range`
    /// and the ranges must be non-overlapping and collectively exhaustive.
    pub version_range: Option<&'static str>,
    /// Glob patterns for matching files.
    pub patterns: &'static [&'static str],
    /// When any of these named checks are active, exclude their patterns from
    /// this check's file list. Used to avoid double-checking files that a
    /// dedicated formatter already owns.
    pub excludes_if_active: &'static [&'static str],
    /// Slow checks are skipped when `--fast` is passed.
    pub slow: bool,
    pub kind: CheckKind,
}

impl Check {
    pub fn has_fix(&self) -> bool {
        match &self.kind {
            CheckKind::Template { fix_cmd, .. } => !fix_cmd.is_empty(),
            CheckKind::Special(SpecialKind::Links) => false,
            CheckKind::Special(SpecialKind::RenovateDeps) => true,
        }
    }

    // --- Constructors ---

    /// Check invoked once per matched file (`{FILE}`). `name` is also used as `bin_name`.
    pub fn file(name: &'static str, check_cmd: &'static str, patterns: &'static [&'static str]) -> Self {
        Self::template(name, patterns, check_cmd, Scope::File)
    }

    /// Check invoked once with all matched files (`{FILES}`). `name` is also used as `bin_name`.
    pub fn files(name: &'static str, check_cmd: &'static str, patterns: &'static [&'static str]) -> Self {
        Self::template(name, patterns, check_cmd, Scope::Files)
    }

    /// Check invoked once per project (no file args). `name` is also used as `bin_name`.
    pub fn project(name: &'static str, check_cmd: &'static str, patterns: &'static [&'static str]) -> Self {
        Self::template(name, patterns, check_cmd, Scope::Project)
    }

    fn template(
        name: &'static str,
        patterns: &'static [&'static str],
        check_cmd: &'static str,
        scope: Scope,
    ) -> Self {
        Check {
            name,
            bin_name: name,
            mise_tool_name: None,
            version_range: None,
            patterns,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd,
                fix_cmd: "",
                scope,
            },
        }
    }

    /// Special check with custom logic (not a simple command template).
    pub fn special(name: &'static str, bin_name: &'static str, kind: SpecialKind) -> Self {
        Check {
            name,
            bin_name,
            mise_tool_name: None,
            version_range: None,
            patterns: &[],
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Special(kind),
        }
    }

    // --- Modifiers ---

    /// Override `bin_name` when the binary name differs from the check name
    /// (e.g. `ruff-format` invokes `ruff`).
    pub fn bin(mut self, bin_name: &'static str) -> Self {
        self.bin_name = bin_name;
        self
    }

    /// Set the mise.toml tool key when the binary ships as part of a toolchain
    /// (e.g. `cargo-fmt` ships with `rust`).
    pub fn mise_tool(mut self, name: &'static str) -> Self {
        self.mise_tool_name = Some(name);
        self
    }

    /// Add a fix command (auto-fix mode).
    pub fn fix(mut self, fix_cmd: &'static str) -> Self {
        if let CheckKind::Template { fix_cmd: ref mut f, .. } = self.kind {
            *f = fix_cmd;
        }
        self
    }

    /// Restrict activation to a semver range of the declared tool version.
    #[allow(dead_code)]
    pub fn version_req(mut self, range: &'static str) -> Self {
        self.version_range = Some(range);
        self
    }

    /// Skip files already owned by the named checks (avoids double-checking).
    pub fn excludes(mut self, names: &'static [&'static str]) -> Self {
        self.excludes_if_active = names;
        self
    }

    /// Mark as slow — skipped when `--fast` is passed.
    pub fn slow(mut self) -> Self {
        self.slow = true;
        self
    }
}

pub fn builtin() -> Vec<Check> {
    vec![
        Check::file("shellcheck", "shellcheck {FILE}", &["*.sh", "*.bash", "*.bats"]),
        Check::file("shfmt", "shfmt -d {FILE}", &["*.sh", "*.bash"]).fix("shfmt -w {FILE}"),
        Check::file("markdownlint", "markdownlint {FILE}", &["*.md"]).fix("markdownlint --fix {FILE}"),
        Check::files("prettier", "prettier --check {FILES}", &["*.md", "*.yml", "*.yaml"]).fix("prettier --write {FILES}"),
        Check::file("actionlint", "actionlint {FILE}", &[".github/workflows/*.yml", ".github/workflows/*.yaml"]),
        Check::file("hadolint", "hadolint {FILE}", &["Dockerfile", "Dockerfile.*", "*.dockerfile"]),
        Check::files("codespell", "codespell {FILES}", &["*"]).fix("codespell --write-changes {FILES}"),
        // Defer to formatters that enforce line length — those are the ones
        // that conflict with ec's max_line_length editorconfig check.
        Check::files("ec", "ec {FILES}", &["*"])
            .excludes(&["cargo-fmt", "ruff-format", "biome-format", "prettier"]),
        Check::project("golangci-lint", "golangci-lint run --new-from-rev={MERGE_BASE}", &["*.go"]),
        Check::file("ruff", "ruff check {FILE}", &["*.py"]).fix("ruff check --fix {FILE}"),
        Check::file("ruff-format", "ruff format --check {FILE}", &["*.py"]).bin("ruff").fix("ruff format {FILE}"),
        Check::file("biome", "biome check {FILE}", &["*.json", "*.jsonc", "*.js", "*.ts", "*.jsx", "*.tsx"]).fix("biome check --fix {FILE}"),
        Check::file("biome-format", "biome format {FILE}", &["*.json", "*.jsonc", "*.js", "*.ts", "*.jsx", "*.tsx"]).bin("biome").fix("biome format --write {FILE}"),
        Check::project("cargo-clippy", "cargo clippy -q -- -D warnings", &["*.rs"])
            .fix("cargo clippy -q --fix --allow-dirty --allow-staged -- -D warnings")
            .mise_tool("rust"),
        Check::project("cargo-fmt", "cargo fmt -- --check", &["*.rs"])
            .fix("cargo fmt")
            .mise_tool("rust"),
        Check::special("lychee", "lychee", SpecialKind::Links),
        Check::special("renovate-deps", "renovate", SpecialKind::RenovateDeps).slow(),
    ]
}

/// Reads `[tools]` from the consuming repo's mise.toml and returns a map of
/// tool name → declared version string.
pub fn read_mise_tools(project_root: &Path) -> HashMap<String, String> {
    let path = project_root.join("mise.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let value: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let mut tools = HashMap::new();
    if let Some(table) = value.get("tools").and_then(|v| v.as_table()) {
        for (name, val) in table {
            let version = match val {
                toml::Value::String(s) => Some(s.clone()),
                toml::Value::Table(t) => {
                    t.get("version").and_then(|v| v.as_str()).map(String::from)
                }
                _ => None,
            };
            if let Some(v) = version {
                tools.insert(name.clone(), v);
            }
        }
    }
    tools
}

/// Returns true if the check's tool is declared in mise.toml and its version
/// satisfies the check's version_range (if any).
pub fn check_active(check: &Check, mise_tools: &HashMap<String, String>) -> bool {
    let lookup_key = check.mise_tool_name.unwrap_or(check.bin_name);
    let Some(declared) = mise_tools.get(lookup_key) else {
        return false;
    };
    let Some(range_str) = check.version_range else {
        return true;
    };
    let Ok(req) = semver::VersionReq::parse(range_str) else {
        return false;
    };
    coerce_version(declared).is_some_and(|v| req.matches(&v))
}

/// Parses a version string, padding with `.0` components if needed to satisfy
/// semver's three-part requirement (e.g. `"20"` → `20.0.0`, `"3.12"` → `3.12.0`).
fn coerce_version(s: &str) -> Option<semver::Version> {
    semver::Version::parse(s).ok().or_else(|| {
        let parts = s.split('.').count();
        match parts {
            1 => semver::Version::parse(&format!("{s}.0.0")).ok(),
            2 => semver::Version::parse(&format!("{s}.0")).ok(),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// If any entry for a bin_name declares a version_range, every entry for that
    /// bin_name must declare one. A mix of ranged and unranged entries for the same
    /// binary is ambiguous — it would be impossible to guarantee exactly one activates.
    /// (Multiple unranged entries for the same binary are fine: they're different
    /// subcommand invocations of the same tool, e.g. `biome check` vs `biome format`.)
    #[test]
    fn version_ranges_must_not_be_mixed_with_unranged_entries() {
        let registry = builtin();
        let mut by_bin: HashMap<&str, Vec<&Check>> = HashMap::new();
        for check in &registry {
            by_bin.entry(check.bin_name).or_default().push(check);
        }
        for (bin, checks) in &by_bin {
            let any_ranged = checks.iter().any(|c| c.version_range.is_some());
            if any_ranged {
                for check in checks {
                    assert!(
                        check.version_range.is_some(),
                        "check '{}' shares bin_name '{}' with version-ranged entries but has no version_range",
                        check.name,
                        bin,
                    );
                }
            }
        }
    }
}
