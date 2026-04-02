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
    /// Binary name to check in PATH.
    pub bin_name: &'static str,
    /// Glob patterns (space-separated) for matching files.
    pub patterns: &'static str,
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
            patterns: "*.sh *.bash *.bats",
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
            patterns: "*.sh *.bash",
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
            patterns: "*.md",
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
            patterns: "*.md *.json *.yml *.yaml",
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
            patterns: ".github/workflows/*.yml .github/workflows/*.yaml",
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
            patterns: "Dockerfile Dockerfile.* *.dockerfile",
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
            patterns: "*",
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
            patterns: "*",
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
            patterns: "*.go",
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
            patterns: "*.py",
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
            patterns: "*.py",
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
            patterns: "*.json *.jsonc *.js *.ts *.jsx *.tsx",
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
            patterns: "*.json *.jsonc *.js *.ts *.jsx *.tsx",
            slow: false,
            kind: CheckKind::Template {
                check_cmd: "biome format {FILE}",
                fix_cmd: "biome format --write {FILE}",
                scope: Scope::File,
            },
        },
        Check {
            name: "links",
            bin_name: "lychee",
            patterns: "",
            slow: false,
            kind: CheckKind::Special(SpecialKind::Links),
        },
        Check {
            name: "renovate-deps",
            bin_name: "renovate",
            patterns: "",
            slow: true,
            kind: CheckKind::Special(SpecialKind::RenovateDeps),
        },
    ]
}
