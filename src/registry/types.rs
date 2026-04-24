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

/// Which init profile a check belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Category {
    /// Primary programming language linter/formatter (Rust, Python, Go, …) — all init profiles.
    Lang,
    /// Supplementary language check (shell, Docker, CI/CD) — `default` + `comprehensive` only.
    Style,
    /// General fast tool (not language-specific) — `default` and `comprehensive` init profiles.
    #[default]
    Default,
    /// Comprehensive-only tool (e.g. expensive or niche checks).
    Slow,
}

/// How a check participates in `--fast-only`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunPolicy {
    /// Always runs, including in `--fast-only`.
    #[default]
    Fast,
    /// Skipped in `--fast-only` unless explicitly named.
    Slow,
    /// Runs in `--fast-only` only when the changed files are relevant to the check.
    Adaptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKind {
    Links,
    RenovateDeps,
    LicenseHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FixBehavior {
    #[default]
    Definitive,
    PartialNeedsVerify,
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
    pub run_policy: RunPolicy,
    /// When set, look for `(filename, flag)` in config_dir: if the file exists, inject
    /// `flag <abs-path>` into the command right after the binary name.
    pub linter_config: Option<(&'static str, &'static str)>,
    /// Environment variables to set when invoking this check's external process.
    pub env: &'static [(&'static str, &'static str)],
    /// Config-like files that affect this check's results and should trigger
    /// a one-time all-files baseline run when changed.
    pub baseline_configs: &'static [ConfigFile],
    /// Known upstream config locations that flint does not support for this
    /// check. Their presence is a hard failure to avoid silent config drift.
    pub unsupported_configs: &'static [ConfigFile],
    /// This check is a formatter — it owns certain file types for formatting purposes.
    pub is_formatter: bool,
    /// Skip files owned by active formatters (used by ec to avoid double-checking).
    pub defers_to_formatters: bool,
    /// Always considered active regardless of mise.toml (used for config-activated checks).
    pub activate_unconditionally: bool,
    /// Toolchain status and optional components for `mise_tool_name`.
    ///
    /// - `None` — `mise_tool_name` is a standalone linter binary.
    /// - `Some(None)` — `mise_tool_name` is a language runtime/SDK (e.g. `go`,
    ///   `dotnet`) with no per-tool components.
    /// - `Some(Some("clippy,rustfmt"))` — toolchain with components; produces an
    ///   inline-table entry like `rust = { version = "latest", components = "…" }`.
    ///
    /// Toolchain keys stay above the `# Linters` header in `mise.toml` so they're
    /// visually separated from lint-only entries.
    pub toolchain: Option<Option<&'static str>>,
    /// On Windows, the binary is a self-executing JAR that cannot be run directly
    /// or via cmd.exe — invoke as `java -jar <resolved-path>` instead.
    pub windows_java_jar: bool,
    pub fix_behavior: FixBehavior,
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

    pub fn fix_behavior(&self) -> FixBehavior {
        if self.has_fix() {
            self.fix_behavior
        } else {
            FixBehavior::Definitive
        }
    }

    /// True when `mise_tool_name` refers to a language runtime/SDK rather than a
    /// standalone linter binary.
    pub fn is_toolchain(&self) -> bool {
        self.toolchain.is_some()
    }

    /// Toolchain components to request when installing via mise, if any
    /// (e.g. `"clippy,rustfmt"` for rust). `None` for non-toolchains and for
    /// toolchains without components.
    pub fn components(&self) -> Option<&'static str> {
        self.toolchain.flatten()
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
            env: &[],
            baseline_configs: &[],
            unsupported_configs: &[],
            is_formatter: false,
            defers_to_formatters: false,
            activate_unconditionally: false,
            category: Category::Default,
            run_policy: RunPolicy::Fast,
            toolchain: None,
            kind: CheckKind::Template {
                check_cmd,
                fix_cmd: "",
                full_cmd: "",
                full_fix_cmd: "",
                scope,
            },
            windows_java_jar: false,
            fix_behavior: FixBehavior::Definitive,
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
            env: &[],
            baseline_configs: &[],
            unsupported_configs: &[],
            is_formatter: false,
            defers_to_formatters: false,
            activate_unconditionally: false,
            category: Category::Default,
            run_policy: RunPolicy::Fast,
            toolchain: None,
            windows_java_jar: false,
            fix_behavior: FixBehavior::Definitive,
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

    pub fn partial_fix(mut self) -> Self {
        self.fix_behavior = FixBehavior::PartialNeedsVerify;
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

    /// On Windows, invoke this binary via `java -jar <path>` rather than directly.
    /// Use for self-executing JARs (e.g. ktlint) that cmd.exe cannot run.
    pub fn windows_java_jar(mut self) -> Self {
        self.windows_java_jar = true;
        self
    }

    /// Mark as comprehensive-only in `flint init`, and skipped by `--fast-only`.
    pub fn slow(mut self) -> Self {
        self.category = Category::Slow;
        self.run_policy = RunPolicy::Slow;
        self
    }

    /// Mark as comprehensive-only in `flint init`, and relevance-gated in `--fast-only`.
    pub fn adaptive(mut self) -> Self {
        self.category = Category::Slow;
        self.run_policy = RunPolicy::Adaptive;
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

    /// Mark as a language-specific style/formatter check — included in the
    /// `default` and `comprehensive` init profiles (not `lang`).
    pub fn style(mut self) -> Self {
        self.category = Category::Style;
        self
    }

    /// Mark `mise_tool_name` as a language runtime/SDK (e.g. `go`, `dotnet`)
    /// with no per-tool components. `flint init` keeps toolchain keys above the
    /// `# Linters` header in `mise.toml`.
    pub fn toolchain(mut self) -> Self {
        self.toolchain = Some(None);
        self
    }

    /// Mark `mise_tool_name` as a language toolchain and request the given
    /// components when installing via `flint init` (e.g. `"clippy,rustfmt"` for
    /// the `rust` toolchain). Produces an inline-table entry like
    /// `rust = { version = "latest", components = "clippy,rustfmt" }`.
    pub fn toolchain_components(mut self, components: &'static str) -> Self {
        self.toolchain = Some(Some(components));
        self
    }

    /// Inject a config file from config_dir into the linter command.
    /// If `config_dir/file` exists at runtime, `flag <abs-path>` is inserted
    /// right after the binary name. Has no effect when the file is absent.
    pub fn linter_config(mut self, file: &'static str, flag: &'static str) -> Self {
        self.linter_config = Some((file, flag));
        self
    }

    /// Set fixed environment variables when spawning this check's process.
    pub fn env(mut self, env: &'static [(&'static str, &'static str)]) -> Self {
        self.env = env;
        self
    }

    pub fn baseline_configs(mut self, files: &'static [ConfigFile]) -> Self {
        self.baseline_configs = files;
        self
    }

    pub fn unsupported_configs(mut self, files: &'static [ConfigFile]) -> Self {
        self.unsupported_configs = files;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConfigBase {
    ProjectRoot,
    ConfigDir,
}

#[derive(Debug, Clone, Copy)]
pub enum ConfigMatch {
    Exists,
    TomlSection(&'static [&'static str]),
    IniSection(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct ConfigFile {
    pub base: ConfigBase,
    pub path: &'static str,
    pub presence: ConfigMatch,
}

impl ConfigFile {
    pub const fn project(path: &'static str) -> Self {
        Self {
            base: ConfigBase::ProjectRoot,
            path,
            presence: ConfigMatch::Exists,
        }
    }

    pub const fn config_dir(path: &'static str) -> Self {
        Self {
            base: ConfigBase::ConfigDir,
            path,
            presence: ConfigMatch::Exists,
        }
    }

    pub const fn project_toml_section(
        path: &'static str,
        section: &'static [&'static str],
    ) -> Self {
        Self {
            base: ConfigBase::ProjectRoot,
            path,
            presence: ConfigMatch::TomlSection(section),
        }
    }

    pub const fn project_ini_section(path: &'static str, section: &'static str) -> Self {
        Self {
            base: ConfigBase::ProjectRoot,
            path,
            presence: ConfigMatch::IniSection(section),
        }
    }
}
