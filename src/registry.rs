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
    /// Glob patterns (space-separated) for matching files.
    pub patterns: &'static str,
    /// When any of these named checks are active, exclude their patterns from
    /// this check's file list. Used to avoid double-checking files that a
    /// dedicated formatter already owns.
    pub excludes_if_active: &'static [&'static str],
    /// Slow checks are skipped when `--fast` is passed.
    pub slow: bool,
    pub kind: CheckKind,
}

impl Check {
    /// The binary name used to check PATH availability.
    pub fn bin(&self) -> &str {
        self.bin_name
    }

    pub fn has_fix(&self) -> bool {
        match &self.kind {
            CheckKind::Template { fix_cmd, .. } => !fix_cmd.is_empty(),
            CheckKind::Special(SpecialKind::Links) => false,
            CheckKind::Special(SpecialKind::RenovateDeps) => true,
        }
    }
}

pub fn builtin() -> Vec<Check> {
    vec![
        Check {
            name: "shellcheck",
            bin_name: "shellcheck",
            mise_tool_name: None,
            patterns: "*.sh *.bash *.bats",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "shellcheck {FILE}",
                fix_cmd: "",
                scope: Scope::File,
            },
        },
        Check {
            name: "shfmt",
            bin_name: "shfmt",
            mise_tool_name: None,
            patterns: "*.sh *.bash",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "shfmt -d {FILE}",
                fix_cmd: "shfmt -w {FILE}",
                scope: Scope::File,
            },
        },
        Check {
            name: "markdownlint",
            bin_name: "markdownlint",
            mise_tool_name: None,
            patterns: "*.md",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "markdownlint {FILE}",
                fix_cmd: "markdownlint --fix {FILE}",
                scope: Scope::File,
            },
        },
        Check {
            name: "prettier",
            bin_name: "prettier",
            mise_tool_name: None,
            patterns: "*.md *.yml *.yaml",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "prettier --check {FILES}",
                fix_cmd: "prettier --write {FILES}",
                scope: Scope::Files,
            },
        },
        Check {
            name: "actionlint",
            bin_name: "actionlint",
            mise_tool_name: None,
            patterns: ".github/workflows/*.yml .github/workflows/*.yaml",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "actionlint {FILE}",
                fix_cmd: "",
                scope: Scope::File,
            },
        },
        Check {
            name: "hadolint",
            bin_name: "hadolint",
            mise_tool_name: None,
            patterns: "Dockerfile Dockerfile.* *.dockerfile",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "hadolint {FILE}",
                fix_cmd: "",
                scope: Scope::File,
            },
        },
        Check {
            name: "codespell",
            bin_name: "codespell",
            mise_tool_name: None,
            patterns: "*",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "codespell {FILES}",
                fix_cmd: "codespell --write-changes {FILES}",
                scope: Scope::Files,
            },
        },
        Check {
            name: "ec",
            bin_name: "ec",
            mise_tool_name: None,
            version_range: None,
            patterns: "*",
            // Defer to formatters that enforce line length — those are the ones
            // that conflict with ec's max_line_length editorconfig check.
            excludes_if_active: &["cargo-fmt", "ruff-format", "biome-format", "prettier"],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "ec {FILES}",
                fix_cmd: "",
                scope: Scope::Files,
            },
        },
        Check {
            name: "golangci-lint",
            bin_name: "golangci-lint",
            mise_tool_name: None,
            patterns: "*.go",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "golangci-lint run --new-from-rev={MERGE_BASE}",
                fix_cmd: "",
                scope: Scope::Project,
            },
        },
        Check {
            name: "ruff",
            bin_name: "ruff",
            mise_tool_name: None,
            patterns: "*.py",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "ruff check {FILE}",
                fix_cmd: "ruff check --fix {FILE}",
                scope: Scope::File,
            },
        },
        Check {
            name: "ruff-format",
            bin_name: "ruff",
            mise_tool_name: None,
            patterns: "*.py",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "ruff format --check {FILE}",
                fix_cmd: "ruff format {FILE}",
                scope: Scope::File,
            },
        },
        Check {
            name: "biome",
            bin_name: "biome",
            mise_tool_name: None,
            patterns: "*.json *.jsonc *.js *.ts *.jsx *.tsx",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "biome check {FILE}",
                fix_cmd: "biome check --fix {FILE}",
                scope: Scope::File,
            },
        },
        Check {
            name: "biome-format",
            bin_name: "biome",
            mise_tool_name: None,
            patterns: "*.json *.jsonc *.js *.ts *.jsx *.tsx",
            version_range: None,
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "biome format {FILE}",
                fix_cmd: "biome format --write {FILE}",
                scope: Scope::File,
            },
        },
        Check {
            name: "cargo-clippy",
            bin_name: "cargo-clippy",
            mise_tool_name: Some("rust"),
            version_range: None,
            patterns: "*.rs",
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "cargo clippy -q -- -D warnings",
                fix_cmd: "cargo clippy -q --fix --allow-dirty --allow-staged -- -D warnings",
                scope: Scope::Project,
            },
        },
        Check {
            name: "cargo-fmt",
            bin_name: "cargo-fmt",
            mise_tool_name: Some("rust"),
            version_range: None,
            patterns: "*.rs",
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "cargo fmt -- --check",
                fix_cmd: "cargo fmt",
                scope: Scope::Project,
            },
        },
        Check {
            name: "links",
            bin_name: "lychee",
            mise_tool_name: None,
            version_range: None,
            patterns: "",
            excludes_if_active: &[],
            slow: false,
            kind: CheckKind::Special(SpecialKind::Links),
        },
        Check {
            name: "renovate-deps",
            bin_name: "renovate",
            mise_tool_name: None,
            version_range: None,
            patterns: "",
            excludes_if_active: &[],
            slow: true,
            kind: CheckKind::Special(SpecialKind::RenovateDeps),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
