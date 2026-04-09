use std::collections::HashMap;
use std::path::Path;

use crate::linters::renovate_deps::RENOVATE_CONFIG_PATTERNS;

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

/// Which init profile (and `--fast-only` behaviour) a check belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Category {
    /// Primary programming language linter/formatter (Rust, Python, Go, …) — all init profiles.
    Lang,
    /// Supplementary language check (shell, Docker, CI/CD) — `default` + `comprehensive` only.
    Style,
    /// General fast tool (not language-specific) — `default` and `comprehensive` init profiles.
    #[default]
    Default,
    /// Slow tool — `comprehensive` init profile only; skipped when `--fast-only` is passed.
    Slow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKind {
    Links,
    RenovateDeps,
    LicenseHeader,
}

#[derive(Debug, Clone)]
pub enum CheckKind {
    Template {
        check_cmd: &'static str,
        fix_cmd: &'static str,
        /// When set and `file_list.full == true`, used instead of `check_cmd` as a
        /// project-wide check command (no `{FILES}` substitution). Useful for tools like
        /// `cargo fmt` that handle all-files scanning better than a long file list.
        full_cmd: &'static str,
        /// When set and `file_list.full == true` in fix mode, used instead of `fix_cmd`.
        full_fix_cmd: &'static str,
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
    pub category: Category,
    /// When set, look for `(filename, flag)` in config_dir: if the file exists, inject
    /// `flag <abs-path>` into the command right after the binary name.
    pub linter_config: Option<(&'static str, &'static str)>,
    /// This check is a formatter — it owns certain file types for formatting purposes.
    pub is_formatter: bool,
    /// Skip files owned by active formatters (used by ec to avoid double-checking).
    pub defers_to_formatters: bool,
    /// Always considered active regardless of mise.toml (used for config-activated checks).
    pub activate_unconditionally: bool,
    /// Canonical mise tool key to write when setting up a new project (e.g. `npm:prettier`).
    /// When `None`, falls back to `mise_tool_name` then `bin_name`.
    /// Distinct from `mise_tool_name` (lookup key) because existing repos may declare the
    /// same tool under a bare name (e.g. `ruff = "latest"`) which must still be detected.
    pub mise_install_key: Option<&'static str>,
    /// Optional mise toolchain components to request when installing via `flint init`
    /// (e.g. `"clippy,rustfmt"` for the `rust` toolchain). Produces an inline-table
    /// entry: `rust = { version = "latest", components = "clippy,rustfmt" }`.
    pub mise_install_components: Option<&'static str>,
    pub kind: CheckKind,
    /// Plain-text description of what the check does — shown in `flint linters` and the README table.
    pub desc: &'static str,
    /// Extended markdown documentation shown in the README detail section (behaviour, config examples).
    pub docs: &'static str,
}

impl Check {
    pub fn has_fix(&self) -> bool {
        match &self.kind {
            CheckKind::Template { fix_cmd, .. } => !fix_cmd.is_empty(),
            CheckKind::Special(SpecialKind::Links) => false,
            CheckKind::Special(SpecialKind::RenovateDeps) => true,
            CheckKind::Special(SpecialKind::LicenseHeader) => false,
        }
    }

    /// Returns false for checks implemented entirely in-process with no external binary.
    pub fn uses_binary(&self) -> bool {
        !matches!(self.kind, CheckKind::Special(SpecialKind::LicenseHeader))
    }

    // --- Constructors ---

    /// Check invoked once per matched file (`{FILE}`). `name` is also used as `bin_name`.
    pub fn file(
        name: &'static str,
        check_cmd: &'static str,
        patterns: &'static [&'static str],
    ) -> Self {
        Self::template(name, patterns, check_cmd, Scope::File)
    }

    /// Check invoked once with all matched files (`{FILES}`). `name` is also used as `bin_name`.
    pub fn files(
        name: &'static str,
        check_cmd: &'static str,
        patterns: &'static [&'static str],
    ) -> Self {
        Self::template(name, patterns, check_cmd, Scope::Files)
    }

    /// Check invoked once per project (no file args). `name` is also used as `bin_name`.
    pub fn project(
        name: &'static str,
        check_cmd: &'static str,
        patterns: &'static [&'static str],
    ) -> Self {
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
            linter_config: None,
            is_formatter: false,
            defers_to_formatters: false,
            activate_unconditionally: false,
            category: Category::Default,
            mise_install_key: None,
            mise_install_components: None,
            kind: CheckKind::Template {
                check_cmd,
                fix_cmd: "",
                full_cmd: "",
                full_fix_cmd: "",
                scope,
            },
            desc: "",
            docs: "",
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
            linter_config: None,
            is_formatter: false,
            defers_to_formatters: false,
            activate_unconditionally: false,
            category: Category::Default,
            mise_install_key: None,
            mise_install_components: None,
            kind: CheckKind::Special(kind),
            desc: "",
            docs: "",
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
        if let CheckKind::Template {
            fix_cmd: ref mut f, ..
        } = self.kind
        {
            *f = fix_cmd;
        }
        self
    }

    /// Set project-wide commands used instead of `check_cmd`/`fix_cmd` when
    /// `file_list.full == true` (explicit `--full` or no merge base). Commands
    /// run with no file arguments — useful for tools that discover files internally
    /// (e.g. `cargo fmt`). Also handles edition detection for Rust tools.
    pub fn full_cmd(mut self, check: &'static str, fix: &'static str) -> Self {
        if let CheckKind::Template {
            full_cmd: ref mut c,
            full_fix_cmd: ref mut f,
            ..
        } = self.kind
        {
            *c = check;
            *f = fix;
        }
        self
    }

    /// Restrict activation to a semver range of the declared tool version.
    #[allow(dead_code)]
    pub fn version_req(mut self, range: &'static str) -> Self {
        self.version_range = Some(range);
        self
    }

    /// Mark as slow — skipped when `--fast-only` is passed; `comprehensive` init profile only.
    pub fn slow(mut self) -> Self {
        self.category = Category::Slow;
        self
    }

    /// Mark as a formatter — files it owns are excluded from ec when both are active.
    pub fn formatter(mut self) -> Self {
        self.is_formatter = true;
        self
    }

    /// Skip files owned by active formatters (for ec — avoids double-checking).
    pub fn defer_to_formatters(mut self) -> Self {
        self.defers_to_formatters = true;
        self
    }

    /// Always considered active regardless of mise.toml (for config-activated checks).
    pub fn activate_unconditionally(mut self) -> Self {
        self.activate_unconditionally = true;
        self
    }

    /// Set the plain-text description shown in `flint linters` and the README table.
    pub fn desc(mut self, desc: &'static str) -> Self {
        self.desc = desc;
        self
    }

    /// Set extended markdown documentation shown in the README detail section.
    pub fn docs(mut self, docs: &'static str) -> Self {
        self.docs = docs;
        self
    }

    /// Override the patterns field (useful for Special checks that need init-detection
    /// patterns but don't use them for file matching at runtime).
    pub fn patterns(mut self, patterns: &'static [&'static str]) -> Self {
        self.patterns = patterns;
        self
    }

    /// Mark as a primary language analysis check — included in all init profiles.
    pub fn lang(mut self) -> Self {
        self.category = Category::Lang;
        self
    }

    /// Mark as a language-specific style/formatter check — included in all init profiles.
    pub fn style(mut self) -> Self {
        self.category = Category::Style;
        self
    }

    /// Set the canonical mise tool key used when installing this linter via `flint init`
    /// (e.g. `npm:prettier`, `pipx:ruff`). Distinct from `mise_tool_name`, which is the
    /// lookup key used to detect the tool in an existing mise.toml — existing repos may
    /// declare the tool under a bare name (`ruff = "latest"`) which is still detectable
    /// via the alias map but must not be overwritten with the prefixed key.
    pub fn install_key(mut self, key: &'static str) -> Self {
        self.mise_install_key = Some(key);
        self
    }

    /// Set toolchain components required when installing via `flint init`
    /// (e.g. `"clippy,rustfmt"` for the `rust` toolchain).
    pub fn install_components(mut self, components: &'static str) -> Self {
        self.mise_install_components = Some(components);
        self
    }

    /// Inject a config file from config_dir into the linter command.
    /// If `config_dir/file` exists at runtime, `flag <abs-path>` is inserted
    /// right after the binary name. Has no effect when the file is absent.
    pub fn linter_config(mut self, file: &'static str, flag: &'static str) -> Self {
        self.linter_config = Some((file, flag));
        self
    }
}

/// Built-in linter registry.
///
/// # Naming convention
///
/// A check's `name` is the last path segment of its mise tool key (after `:` or `/`):
/// - `editorconfig-checker` → name `editorconfig-checker` (not the binary `ec`)
/// - `npm:markdownlint-cli2` → name `markdownlint-cli2`
/// - `github:pinterest/ktlint` → name `ktlint`
///
/// Exception: when the mise tool key is a language toolchain shared across multiple
/// binaries (e.g. `rust`, `go`, `dotnet`), use the binary name instead — the toolchain
/// name would be ambiguous (`rust` can't name both `cargo-fmt` and `cargo-clippy`).
pub fn builtin() -> Vec<Check> {
    vec![
        Check::file(
            "shellcheck",
            "shellcheck {FILE}",
            &["*.sh", "*.bash", "*.bats"],
        )
        .linter_config(".shellcheckrc", "--rcfile")
        .desc("Lint shell scripts for common mistakes")
        .style(),
        Check::file("shfmt", "shfmt -d {FILE}", &["*.sh", "*.bash"])
            .fix("shfmt -w {FILE}")
            .formatter()
            .desc("Format shell scripts")
            .style(),
        Check::file("markdownlint-cli2", "markdownlint-cli2 {FILE}", &["*.md"])
            .fix("markdownlint-cli2 --fix {FILE}")
            .linter_config(".markdownlint.jsonc", "--config")
            .desc("Lint Markdown files for style and consistency")
            .install_key("npm:markdownlint-cli2"),
        Check::files(
            "prettier",
            "prettier --check {FILES}",
            &["*.md", "*.yml", "*.yaml"],
        )
        .fix("prettier --write {FILES}")
        .full_cmd("prettier --check {ROOT}", "prettier --write {ROOT}")
        .linter_config(".prettierrc", "--config")
        .formatter()
        .desc("Format Markdown and YAML files")
        .install_key("npm:prettier"),
        Check::file(
            "actionlint",
            "actionlint {FILE}",
            &[".github/workflows/*.yml", ".github/workflows/*.yaml"],
        )
        .linter_config("actionlint.yml", "-config-file")
        .desc("Lint GitHub Actions workflow files")
        .style(),
        Check::file(
            "hadolint",
            "hadolint {FILE}",
            &["Dockerfile", "Dockerfile.*", "*.dockerfile"],
        )
        .linter_config(".hadolint.yaml", "--config")
        .desc("Lint Dockerfiles")
        .style(),
        Check::files("codespell", "codespell {FILES}", &["*"])
            .fix("codespell --write-changes {FILES}")
            .linter_config(".codespellrc", "--config")
            .desc("Check for common spelling mistakes")
            .install_key("pipx:codespell"),
        // Defer to formatters that enforce line length — those are the ones
        // that conflict with ec's max_line_length editorconfig check.
        // Note: ec's -config flag controls ec's own JSON config, not .editorconfig itself.
        Check::files("editorconfig-checker", "ec {FILES}", &["*"])
            .bin("ec")
            .mise_tool("editorconfig-checker")
            .defer_to_formatters()
            .linter_config(".editorconfig-checker.json", "-config")
            .desc("Check files comply with EditorConfig settings"),
        Check::project(
            "golangci-lint",
            "golangci-lint run --new-from-rev={MERGE_BASE}",
            &["*.go"],
        )
        .linter_config(".golangci.yml", "--config")
        .desc("Lint Go code; uses --new-from-rev to scope analysis to changed code")
        .lang(),
        Check::file("ruff", "ruff check {FILE}", &["*.py"])
            .fix("ruff check --fix {FILE}")
            .linter_config("ruff.toml", "--config")
            .desc("Lint Python code")
            .install_key("pipx:ruff")
            .lang(),
        Check::file("ruff-format", "ruff format --check {FILE}", &["*.py"])
            .bin("ruff")
            .fix("ruff format {FILE}")
            .linter_config("ruff.toml", "--config")
            .formatter()
            .desc("Format Python code")
            .install_key("pipx:ruff")
            .lang(),
        Check::file(
            "biome",
            "biome check {FILE}",
            &["*.json", "*.jsonc", "*.js", "*.ts", "*.jsx", "*.tsx"],
        )
        .fix("biome check --fix {FILE}")
        .desc("Lint JS/TS/JSON files")
        .install_key("npm:@biomejs/biome")
        .lang(),
        Check::file(
            "biome-format",
            "biome format {FILE}",
            &["*.json", "*.jsonc", "*.js", "*.ts", "*.jsx", "*.tsx"],
        )
        .bin("biome")
        .fix("biome format --write {FILE}")
        .formatter()
        .desc("Format JS/TS/JSON files")
        .install_key("npm:@biomejs/biome")
        .lang(),
        Check::project("cargo-clippy", "cargo clippy -q -- -D warnings", &["*.rs"])
            .fix("cargo clippy -q --fix --allow-dirty --allow-staged -- -D warnings")
            .mise_tool("rust")
            .install_components("clippy")
            .desc("Lint Rust code; runs on all .rs files, not just changed")
            .lang(),
        Check::project("cargo-fmt", "cargo fmt -- --check", &["*.rs"])
            .fix("cargo fmt")
            .bin("rustfmt")
            .mise_tool("rust")
            .install_components("rustfmt")
            .formatter()
            .desc("Format Rust code; runs on all .rs files, not just changed")
            .lang(),
        Check::file("gofmt", "gofmt -d {FILE}", &["*.go"])
            .fix("gofmt -w {FILE}")
            .mise_tool("go")
            .formatter()
            .desc("Format Go code")
            .lang(),
        Check::files(
            "google-java-format",
            "google-java-format --dry-run --set-exit-if-changed {FILES}",
            &["*.java"],
        )
        .fix("google-java-format -i {FILES}")
        .mise_tool("github:google/google-java-format")
        .formatter()
        .desc("Format Java code")
        .lang(),
        Check::files(
            "ktlint",
            "ktlint --log-level=error {FILES}",
            &["*.kt", "*.kts"],
        )
        .fix("ktlint --format --log-level=error {FILES}")
        .full_cmd(
            "ktlint --log-level=error {ROOT}",
            "ktlint --format --log-level=error {ROOT}",
        )
        .mise_tool("github:pinterest/ktlint")
        .bin(if cfg!(windows) {
            "ktlint.bat"
        } else {
            "ktlint"
        })
        .formatter()
        .desc("Lint and format Kotlin code")
        .lang(),
        Check::files(
            "dotnet-format",
            "dotnet format --verify-no-changes --include {RELFILES}",
            &["*.cs"],
        )
        .fix("dotnet format --include {RELFILES}")
        .full_cmd("dotnet format --verify-no-changes", "dotnet format")
        .bin("dotnet")
        .mise_tool("dotnet")
        .formatter()
        .desc("Format C# code")
        .lang(),
        Check::special("lychee", "lychee", SpecialKind::Links)
            .desc("Check for broken links")
            .docs(
                "Orchestrates [lychee](https://lychee.cli.rs/) for link checking. \
                Requires `lychee` in `[tools]`.\n\
                \n\
                Default behavior: checks all links in changed files. \
                When `check_all_local = true` in `flint.toml`, adds a second pass \
                over local links in all files — useful when broken internal links \
                from unchanged files also matter.\n\
                \n\
                Configure via `flint.toml`:\n\
                \n\
                ```toml\n\
                [checks.links]\n\
                config = \".github/config/lychee.toml\"\n\
                check_all_local = true\n\
                ```",
            ),
        Check::special("renovate-deps", "renovate", SpecialKind::RenovateDeps)
            .mise_tool("npm:renovate")
            .patterns(RENOVATE_CONFIG_PATTERNS)
            .desc("Verify Renovate dependency snapshot is up to date")
            .docs(
                "Verifies `.github/renovate-tracked-deps.json` is up to date by running \
                Renovate locally and comparing its output against the committed snapshot. \
                Requires `renovate` in `[tools]`.\n\
                \n\
                With `--fix`, automatically regenerates and commits the snapshot.\n\
                \n\
                Configure via `flint.toml`:\n\
                \n\
                ```toml\n\
                [checks.renovate-deps]\n\
                exclude_managers = [\"github-actions\", \"github-runners\"]\n\
                ```",
            ),
        Check::special(
            "license-header",
            "license-header",
            SpecialKind::LicenseHeader,
        )
        .activate_unconditionally()
        .desc("Check source files have the required license header"),
    ]
}

/// Mise tool keys that are no longer supported by flint and should be removed
/// during `flint init`. Each entry is `(old_key, replacement_key)` where
/// `replacement_key` is the modern equivalent that the registry now uses.
pub const OBSOLETE_KEYS: &[(&str, &str)] = &[
    // markdownlint-cli was superseded by markdownlint-cli2 (actively maintained,
    // faster, supports the same config files). flint only supports the cli2 variant.
    ("npm:markdownlint-cli", "npm:markdownlint-cli2"),
];

/// Reads `[tools]` from the consuming repo's mise.toml and returns a map of
/// tool name → declared version string.
///
/// Also registers normalized aliases for backend-prefixed tools so that checks
/// can match by their bare package/binary name. For example:
/// - `"npm:prettier"` → also registers `"prettier"`
/// - `"npm:@biomejs/biome"` → also registers `"biome"` (last path component)
/// - `"github:google/google-java-format"` → also registers `"google-java-format"`
///
/// The original key is always preserved; aliases only fill in missing entries.
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
    // Add normalized aliases: strip the backend prefix (e.g. "npm:", "pipx:", "ubi:")
    // and take the last path component (e.g. "@biomejs/biome" → "biome").
    // Aliases never override an explicitly declared entry.
    let aliases: Vec<(String, String)> = tools
        .iter()
        .filter_map(|(k, v)| {
            let (_, rest) = k.split_once(':')?;
            let base = rest.rsplit('/').next().unwrap_or(rest);
            Some((base.to_string(), v.clone()))
        })
        .collect();
    for (alias, version) in aliases {
        tools.entry(alias).or_insert(version);
    }
    tools
}

/// Returns true if the check's tool is declared in mise.toml and its version
/// satisfies the check's version_range (if any).
pub fn check_active(check: &Check, mise_tools: &HashMap<String, String>) -> bool {
    if check.activate_unconditionally {
        return true;
    }
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

    /// Checks that every linter in the registry that uses an external binary
    /// actually has that binary on PATH. Covers all registry entries, not just
    /// those active in this repo — so tools like ktlint and hadolint are checked
    /// even if they are not declared in this repo's mise.toml.
    ///
    /// This test will fail on machines where not all linter tools are installed,
    /// which is intentional: it identifies what is missing.
    #[test]
    fn all_registry_binaries_found() {
        let registry = builtin();

        let not_found: Vec<&str> = registry
            .iter()
            .filter(|c| c.uses_binary())
            .filter(|c| !binary_on_path(c.bin_name))
            .map(|c| c.name)
            .collect();

        assert!(
            not_found.is_empty(),
            "registry linters missing binary on PATH: {}",
            not_found.join(", ")
        );
    }

    /// Verifies the README linter table is in sync with the registry.
    /// Every column is checked against the registry except `config_file`, which
    /// may contain hand-written footnotes or prose (e.g. the lychee config note).
    ///
    /// Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate.
    #[test]
    fn readme_linter_table_in_sync() {
        let readme_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("README.md must be readable");
        let registry = builtin();

        let expected = generate_readme_table(&registry);

        if std::env::var("UPDATE_README").is_ok() {
            let updated = replace_readme_table(&readme, &expected);
            std::fs::write(&readme_path, updated).expect("failed to write README.md");
            return;
        }

        // Normalize both sides: strip blank lines that prettier adds around
        // headings, tables, and code blocks. This keeps the comparison stable
        // even when docs contain multi-paragraph content with blank lines.
        let actual = extract_readme_table(&readme);
        let expected_norm = strip_blank_lines(&expected);
        if actual != expected_norm {
            panic!(
                "README linter table is out of sync with the registry.\n\
                 Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate.\n\n\
                 Expected:\n{expected_norm}\n\nActual:\n{actual}"
            );
        }
    }

    const README_TABLE_START: &str = "<!-- registry-table-start -->";
    const README_TABLE_END: &str = "<!-- registry-table-end -->";

    fn strip_blank_lines(s: &str) -> String {
        s.lines()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn extract_readme_table(readme: &str) -> String {
        let start = readme
            .find(README_TABLE_START)
            .expect("README missing <!-- registry-table-start --> marker")
            + README_TABLE_START.len();
        let end = readme
            .find(README_TABLE_END)
            .expect("README missing <!-- registry-table-end --> marker");
        // Strip blank lines that prettier inserts around headings, tables, and
        // code blocks — and that linter docs contain between paragraphs.
        strip_blank_lines(&readme[start..end])
    }

    fn replace_readme_table(readme: &str, table: &str) -> String {
        // `start` points just after the opening marker; `&readme[..start]` includes it.
        let start = readme
            .find(README_TABLE_START)
            .expect("README missing <!-- registry-table-start --> marker")
            + README_TABLE_START.len();
        let end = readme
            .find(README_TABLE_END)
            .expect("README missing <!-- registry-table-end --> marker");
        format!(
            "{}\n{}\n{}{}",
            &readme[..start],
            table,
            README_TABLE_END,
            &readme[end + README_TABLE_END.len()..]
        )
    }

    fn generate_readme_table(registry: &[Check]) -> String {
        let generated_comment = "<!-- Generated. Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate. -->";

        // Summary table: Name | Description | Fix
        let headers = ["Name", "Description", "Fix"];
        let rows: Vec<[String; 3]> = registry.iter().map(summary_row).collect();

        let mut widths = headers.map(|h| h.len());
        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }
        let fmt_row = |cells: &[&str]| -> String {
            let cols: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(i, cell)| format!("{:<width$}", cell, width = widths[i]))
                .collect();
            format!("| {} |", cols.join(" | "))
        };
        let separator: Vec<String> = widths.iter().map(|&w| "-".repeat(w)).collect();
        let sep_row = format!("| {} |", separator.join(" | "));
        let header_strs: Vec<&str> = headers.iter().copied().collect();

        let mut lines = vec![
            generated_comment.to_string(),
            fmt_row(&header_strs),
            sep_row,
        ];
        for row in &rows {
            let strs: Vec<&str> = row.iter().map(|s| s.as_str()).collect();
            lines.push(fmt_row(&strs));
        }

        // Per-linter detail sections
        for check in registry {
            lines.push(format!("#### `{}`", check.name));
            lines.push(detail_table(check));
        }

        lines.join("\n")
    }

    fn summary_row(check: &Check) -> [String; 3] {
        let name = format!("`{}`", check.name);
        let desc = if check.desc.is_empty() {
            "—".to_string()
        } else {
            check.desc.to_string()
        };
        let fix = if check.has_fix() { "yes" } else { "—" }.to_string();
        [name, desc, fix]
    }

    fn detail_table(check: &Check) -> String {
        let rows = detail_rows(check);

        let col1_w = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        let col2_w = rows.iter().map(|(_, v)| v.len()).max().unwrap_or(0);

        let fmt = |k: &str, v: &str| format!("| {:<col1_w$} | {:<col2_w$} |", k, v);
        let sep = format!("| {} | {} |", "-".repeat(col1_w), "-".repeat(col2_w));

        // Empty header row: markdown requires one, but we don't need visible
        // column labels — Description and Fix are data rows, not headers.
        let mut lines = vec![fmt("", ""), sep];
        for (k, v) in &rows {
            lines.push(fmt(k, v));
        }
        if !check.docs.is_empty() {
            lines.push(check.docs.to_string());
        }
        lines.join("\n")
    }

    fn detail_rows(check: &Check) -> Vec<(&'static str, String)> {
        let mut rows: Vec<(&'static str, String)> = vec![];

        if !check.desc.is_empty() {
            rows.push(("Description", check.desc.to_string()));
        }

        rows.push((
            "Fix",
            if check.has_fix() { "yes" } else { "no" }.to_string(),
        ));

        let binary = if check.uses_binary() {
            format!("`{}`", check.bin_name)
        } else {
            "(built-in)".to_string()
        };
        rows.push(("Binary", binary));

        let scope = match &check.kind {
            CheckKind::Template { scope, .. } => match scope {
                Scope::File => "file",
                Scope::Files => "files",
                Scope::Project => "project",
            },
            CheckKind::Special(_) => "special",
        };
        rows.push(("Scope", format!("[{scope}](#scopes)")));

        if !check.patterns.is_empty() {
            rows.push(("Patterns", format!("`{}`", check.patterns.join(" "))));
        }

        match check.linter_config {
            Some((filename, _)) => rows.push(("Config", format!("`{filename}`"))),
            None => {
                if matches!(&check.kind, CheckKind::Special(SpecialKind::Links)) {
                    rows.push(("Config", "via `[checks.links]` in flint.toml".to_string()));
                }
            }
        }

        if check.category == Category::Slow {
            rows.push(("Slow", "yes — skipped by `--fast-only`".to_string()));
        }

        rows
    }

    /// Smoke test: every check whose tool key resolves in this repo's expanded
    /// mise_tools map must pass check_active. This catches tool-name mismatches
    /// (wrong lookup key) and version-range violations without a hardcoded list —
    /// new registry entries are covered automatically.
    #[test]
    fn all_flint_repo_linters_detected() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mise_tools = read_mise_tools(project_root);
        let registry = builtin();

        let inactive: Vec<&str> = registry
            .iter()
            .filter(|c| {
                // A check is "expected" if its lookup key appears in the expanded
                // mise_tools map, or if it activates unconditionally.
                c.activate_unconditionally || {
                    let lookup = c.mise_tool_name.unwrap_or(c.bin_name);
                    mise_tools.contains_key(lookup)
                }
            })
            .filter(|c| !check_active(c, &mise_tools))
            .map(|c| c.name)
            .collect();

        assert!(
            inactive.is_empty(),
            "linters not detected in flint repo: {}",
            inactive.join(", ")
        );
    }
}
