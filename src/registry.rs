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

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    /// Command template for check mode.
    pub check_cmd: &'static str,
    /// Command template for fix mode (empty string = no fix support).
    pub fix_cmd: &'static str,
    /// Glob patterns (space-separated) for matching files.
    pub patterns: &'static str,
    pub scope: Scope,
}

impl Check {
    /// The binary name (first word of check_cmd).
    pub fn bin(&self) -> &str {
        self.check_cmd
            .split_whitespace()
            .next()
            .unwrap_or(self.name)
    }

    pub fn has_fix(&self) -> bool {
        !self.fix_cmd.is_empty()
    }
}

pub fn builtin() -> Vec<Check> {
    vec![
        Check {
            name: "shellcheck",
            check_cmd: "shellcheck {FILE}",
            fix_cmd: "",
            patterns: "*.sh *.bash *.bats",
            scope: Scope::File,
        },
        Check {
            name: "shfmt",
            check_cmd: "shfmt -d {FILE}",
            fix_cmd: "shfmt -w {FILE}",
            patterns: "*.sh *.bash",
            scope: Scope::File,
        },
        Check {
            name: "markdownlint",
            check_cmd: "markdownlint {FILE}",
            fix_cmd: "markdownlint --fix {FILE}",
            patterns: "*.md",
            scope: Scope::File,
        },
        Check {
            name: "prettier",
            check_cmd: "prettier --check {FILES}",
            fix_cmd: "prettier --write {FILES}",
            patterns: "*.md *.json *.yml *.yaml",
            scope: Scope::Files,
        },
        Check {
            name: "actionlint",
            check_cmd: "actionlint {FILE}",
            fix_cmd: "",
            patterns: ".github/workflows/*.yml .github/workflows/*.yaml",
            scope: Scope::File,
        },
        Check {
            name: "hadolint",
            check_cmd: "hadolint {FILE}",
            fix_cmd: "",
            patterns: "Dockerfile Dockerfile.* *.dockerfile",
            scope: Scope::File,
        },
        Check {
            name: "codespell",
            check_cmd: "codespell {FILES}",
            fix_cmd: "codespell --write-changes {FILES}",
            patterns: "*",
            scope: Scope::Files,
        },
        Check {
            name: "ec",
            check_cmd: "ec {FILES}",
            fix_cmd: "",
            patterns: "*",
            scope: Scope::Files,
        },
        Check {
            name: "golangci-lint",
            check_cmd: "golangci-lint run --new-from-rev={MERGE_BASE}",
            fix_cmd: "",
            patterns: "*.go",
            scope: Scope::Project,
        },
        Check {
            name: "ruff",
            check_cmd: "ruff check {FILE}",
            fix_cmd: "ruff check --fix {FILE}",
            patterns: "*.py",
            scope: Scope::File,
        },
        Check {
            name: "ruff-format",
            check_cmd: "ruff format --check {FILE}",
            fix_cmd: "ruff format {FILE}",
            patterns: "*.py",
            scope: Scope::File,
        },
        Check {
            name: "biome",
            check_cmd: "biome check {FILE}",
            fix_cmd: "biome check --fix {FILE}",
            patterns: "*.json *.jsonc *.js *.ts *.jsx *.tsx",
            scope: Scope::File,
        },
        Check {
            name: "biome-format",
            check_cmd: "biome format {FILE}",
            fix_cmd: "biome format --write {FILE}",
            patterns: "*.json *.jsonc *.js *.ts *.jsx *.tsx",
            scope: Scope::File,
        },
    ]
}
