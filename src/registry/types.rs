use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use crate::config::Config;
use crate::files::FileList;

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

impl Scope {
    pub fn name(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Files => "files",
            Self::Project => "project",
        }
    }
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

/// The kind of responsibility a check has when multiple checks inspect a file.
///
/// This is deliberately separate from [`Scope`]: a file-scoped check can still
/// be semantic, and a project-scoped check can still be a generic validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ownership {
    /// General syntax, spelling, or style validation.
    Generic,
    /// The check owns formatting for the files it matches.
    Formatter,
    /// Domain-specific policy or security validation.
    Semantic,
    /// Repository-wide or build-graph-oriented validation.
    Project,
}

impl Ownership {
    pub fn name(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Formatter => "formatter",
            Self::Semantic => "semantic",
            Self::Project => "project",
        }
    }
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Self::Lang => "lang",
            Self::Style => "style",
            Self::Default => "default",
            Self::Slow => "slow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OverviewSection {
    Languages,
    FilesFormats,
    ToolingCi,
    General,
}

impl OverviewSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::Languages => "Languages",
            Self::FilesFormats => "Files / Formats",
            Self::ToolingCi => "Tooling / CI",
            Self::General => "General",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverviewRole {
    Linter,
    Formatter,
    Check,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverviewEntry {
    pub section: OverviewSection,
    pub row_name: &'static str,
    pub role: OverviewRole,
    pub description: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeCheckRef {
    native: &'static dyn NativeCheck,
}

impl NativeCheckRef {
    fn new(native: &'static dyn NativeCheck) -> Self {
        Self { native }
    }

    pub fn has_fix(self) -> bool {
        self.native.has_fix()
    }

    pub fn uses_binary(self) -> bool {
        self.native.uses_binary()
    }

    pub fn prepare(self, ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>> {
        self.native.prepare(ctx)
    }

    pub fn config_display(self) -> Option<&'static str> {
        self.native.config_display()
    }

    pub fn is_setup(self) -> bool {
        self.native.is_setup()
    }
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
    Native(NativeCheckRef),
}

impl CheckKind {
    pub fn scope_name(&self) -> &'static str {
        match self {
            Self::Template { scope, .. } => scope.name(),
            Self::Native(_) => "native",
        }
    }

    pub fn is_native(&self) -> bool {
        matches!(self, Self::Native(_))
    }

    pub fn is_setup(&self) -> bool {
        match self {
            Self::Template { .. } => false,
            Self::Native(native) => native.is_setup(),
        }
    }

    pub fn native_config_display(&self) -> Option<&'static str> {
        match self {
            Self::Template { .. } => None,
            Self::Native(native) => native.config_display(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LinterConfig {
    File {
        file: &'static str,
        flag: &'static str,
    },
    DirIfAny {
        files: &'static [&'static str],
        flag: &'static str,
    },
}

impl LinterConfig {
    pub fn display_name(&self) -> String {
        match self {
            Self::File { file, .. } => (*file).to_string(),
            Self::DirIfAny { files, .. } => files.join(" / "),
        }
    }

    pub fn canonical_location(&self) -> String {
        match self {
            Self::File { file, .. } => format!("FLINT_CONFIG_DIR/{file}"),
            Self::DirIfAny { files, .. } => {
                format!("FLINT_CONFIG_DIR (with one of: {})", files.join(", "))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorconfigDirectiveStyle {
    Html,
    Slash,
    Hash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorconfigLineLengthPolicy {
    Default,
    DisableForPatterns {
        patterns: &'static [&'static str],
        comment: &'static str,
        directive_style: Option<EditorconfigDirectiveStyle>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolKeyMigration {
    pub old_key: &'static str,
}

pub trait InitHookContext {
    fn project_root(&self) -> &Path;
    fn config_dir(&self) -> &Path;
    fn line_length(&self) -> u16;
    fn flint_toml_generated(&self) -> bool;
}

pub type InitHookFn = fn(&dyn InitHookContext) -> anyhow::Result<bool>;

/// Output from a single linter run.
pub struct LinterOutput {
    pub ok: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub setup_outcome: Option<SetupOutcome>,
}

impl LinterOutput {
    pub fn err(stderr: impl Into<Vec<u8>>) -> Self {
        Self {
            ok: false,
            stdout: vec![],
            stderr: stderr.into(),
            setup_outcome: None,
        }
    }

    pub fn setup_err(setup_outcome: SetupOutcome, stderr: impl Into<Vec<u8>>) -> Self {
        Self {
            ok: false,
            stdout: vec![],
            stderr: stderr.into(),
            setup_outcome: Some(setup_outcome),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupOutcome {
    Clean,
    NonBlocking,
    Blocking,
    Fatal,
}

impl SetupOutcome {
    pub fn at_least(self, other: Self) -> Self {
        match (self, other) {
            (Self::Fatal, _) | (_, Self::Fatal) => Self::Fatal,
            (Self::Blocking, _) | (_, Self::Blocking) => Self::Blocking,
            (Self::NonBlocking, _) | (_, Self::NonBlocking) => Self::NonBlocking,
            (Self::Clean, Self::Clean) => Self::Clean,
        }
    }
}

pub trait CheckType: Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    fn init_hook(&self) -> Option<InitHookFn> {
        None
    }
    fn native_check(&'static self) -> Option<&'static dyn NativeCheck> {
        None
    }
}

pub struct NativePrepareContext<'a> {
    pub name: &'static str,
    pub file_list: &'a FileList,
    pub project_root: &'a Path,
    pub cfg: &'a Config,
    pub config_dir: &'a Path,
}

pub struct NativeRunContext {
    pub fix: bool,
    pub verbose: bool,
    pub project_root: PathBuf,
}

pub type NativeRunFuture = Pin<Box<dyn Future<Output = LinterOutput> + Send>>;

pub trait PreparedNativeCheck: Send + std::fmt::Debug {
    fn name(&self) -> &str;
    fn tracked_files(&self) -> &[PathBuf] {
        &[]
    }
    fn run(self: Box<Self>, ctx: NativeRunContext) -> NativeRunFuture;
}

pub type NativePrepareFn = fn(NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>>;

pub trait NativeCheck: Sync + std::fmt::Debug {
    fn prepare(&self, ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>>;
    fn has_fix(&self) -> bool {
        false
    }
    fn bin_name(&self) -> Option<&'static str> {
        None
    }
    fn config_display(&self) -> Option<&'static str> {
        None
    }
    fn is_setup(&self) -> bool {
        false
    }
    fn uses_binary(&self) -> bool {
        self.bin_name().is_some()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NativeCheckDef {
    has_fix: bool,
    bin_name: Option<&'static str>,
    config_display: Option<&'static str>,
    setup: bool,
    prepare: NativePrepareFn,
}

impl NativeCheckDef {
    pub const fn new(prepare: NativePrepareFn) -> Self {
        Self {
            has_fix: false,
            bin_name: None,
            config_display: None,
            setup: false,
            prepare,
        }
    }

    pub const fn with_bin(bin_name: &'static str, prepare: NativePrepareFn) -> Self {
        Self {
            has_fix: false,
            bin_name: Some(bin_name),
            config_display: None,
            setup: false,
            prepare,
        }
    }

    pub const fn with_fix(mut self) -> Self {
        self.has_fix = true;
        self
    }

    pub const fn with_config_display(mut self, config_display: &'static str) -> Self {
        self.config_display = Some(config_display);
        self
    }

    pub const fn setup(mut self) -> Self {
        self.setup = true;
        self
    }
}

impl NativeCheck for NativeCheckDef {
    fn prepare(&self, ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>> {
        (self.prepare)(ctx)
    }

    fn has_fix(&self) -> bool {
        self.has_fix
    }

    fn bin_name(&self) -> Option<&'static str> {
        self.bin_name
    }

    fn config_display(&self) -> Option<&'static str> {
        self.config_display
    }

    fn is_setup(&self) -> bool {
        self.setup
    }
}

pub struct CheckTypeDef {
    name: &'static str,
    init_hook: Option<InitHookFn>,
    native: Option<NativeCheckDef>,
}

impl CheckTypeDef {
    pub const fn with_init_hook(name: &'static str, init_hook: InitHookFn) -> Self {
        Self {
            name,
            init_hook: Some(init_hook),
            native: None,
        }
    }

    pub const fn native(name: &'static str, native: NativeCheckDef) -> Self {
        Self {
            name,
            init_hook: None,
            native: Some(native),
        }
    }

    pub const fn native_with_init_hook(
        name: &'static str,
        native: NativeCheckDef,
        init_hook: InitHookFn,
    ) -> Self {
        Self {
            name,
            init_hook: Some(init_hook),
            native: Some(native),
        }
    }
}

impl CheckType for CheckTypeDef {
    fn name(&self) -> &'static str {
        self.name
    }

    fn init_hook(&self) -> Option<InitHookFn> {
        self.init_hook
    }

    fn native_check(&'static self) -> Option<&'static dyn NativeCheck> {
        self.native
            .as_ref()
            .map(|native| native as &dyn NativeCheck)
    }
}

impl std::fmt::Debug for CheckTypeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckTypeDef")
            .field("name", &self.name)
            .field("native", &self.native)
            .finish()
    }
}

pub trait AdaptiveRelevanceContext {
    fn file_list(&self) -> &FileList;
    fn project_root(&self) -> &Path;
}

pub type AdaptiveRelevanceHook = fn(&dyn AdaptiveRelevanceContext) -> bool;

pub trait StatusContext {
    fn config(&self) -> &Config;
}

pub type StatusHook = fn(&dyn StatusContext) -> Option<&'static str>;

pub type NonverboseFailureOutputHook = fn(&[String], &[u8], &[u8]) -> (Vec<u8>, Vec<u8>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingComponentHint {
    pub component: &'static str,
    pub stderr_contains: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowSetup {
    RustComponents,
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
    /// When set, look for linter config in `config_dir` and inject an argument
    /// right after the binary name.
    pub linter_config: Option<LinterConfig>,
    /// Environment variable overrides to apply only in non-verbose runs when
    /// invoking this check's external process. These are intentionally not set
    /// under `--verbose`, so checks must not rely on them always being present.
    pub env: &'static [(&'static str, &'static str)],
    /// Line prefixes to drop from stdout/stderr in non-verbose mode. This is
    /// only for low-value noise; actionable diagnostics must remain visible.
    pub nonverbose_filter_prefixes: &'static [&'static str],
    /// Line prefixes to drop from stderr in non-verbose mode. This is only for
    /// low-value log noise; actionable diagnostics must remain visible.
    pub stderr_filter_prefixes: &'static [&'static str],
    /// Config-like file that affects this check's results and should trigger
    /// a one-time all-files baseline run when changed.
    pub baseline_config: Option<ConfigFile>,
    /// Known upstream config locations that flint does not support for this
    /// check. Their presence is a hard failure to avoid silent config drift.
    pub unsupported_configs: &'static [ConfigFile],
    /// When true, do not treat an unsupported config entry as unsupported if it
    /// resolves to the same path as this check's supported baseline config.
    pub allow_baseline_overlap_in_unsupported_configs: bool,
    /// Old mise tool keys that should migrate to this check's current install key.
    pub tool_key_migrations: Vec<ToolKeyMigration>,
    /// Optional check-type behavior shared by related checks.
    pub check_type: Option<&'static dyn CheckType>,
    /// Optional relevance hook. When set, the check is skipped on filtered
    /// (local-default) runs unless the hook reports the changed files relevant.
    pub adaptive_relevance: Option<AdaptiveRelevanceHook>,
    /// Optional status override shown by `flint linters`.
    pub status_hook: Option<StatusHook>,
    /// Optional output normalizer used for non-verbose failing process runs.
    pub nonverbose_failure_output: Option<NonverboseFailureOutputHook>,
    /// Output markers that make an otherwise successful process invocation fail.
    ///
    /// Some tools are configured to report violations with a zero exit status
    /// (for example, Checkstyle with warning severity), while their host build
    /// plugin treats that output as a failure. These markers preserve that
    /// repository-level contract in Flint.
    pub failure_output_patterns: &'static [&'static str],
    /// Optional hint appended when a known toolchain component is missing.
    pub missing_component_hint: Option<MissingComponentHint>,
    /// Additional config-like files that trigger an all-files baseline run when changed.
    pub baseline_triggers: &'static [ConfigFile],
    /// This check is a formatter — it owns certain file types for formatting purposes.
    pub is_formatter: bool,
    /// Broad responsibility category used to document legitimate overlap.
    pub ownership: Ownership,
    /// Skip files owned by active formatters (used by ec to avoid double-checking).
    pub defers_to_formatters: bool,
    /// Optional `.editorconfig` line-length carve-out owned by this check.
    pub editorconfig_line_length_policy: EditorconfigLineLengthPolicy,
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
    /// The binary is a self-executing JAR that must be invoked as
    /// `java -jar <resolved-path>` instead of directly.
    pub java_jar: bool,
    /// Extra generated workflow setup needed when this check is selected by `flint init`.
    pub workflow_setup: Option<WorkflowSetup>,
    pub fix_behavior: FixBehavior,
    /// Deterministic order for fix-capable checks. Checks that can touch the
    /// same file must declare an explicit order; fixes are still run serially.
    pub fix_order: Option<u16>,
    pub kind: CheckKind,
    /// Plain-text description of what the check does — shown in `flint linters` and the README table.
    pub desc: &'static str,
    /// Upstream project page used when rendering linter names in generated docs.
    pub project_url: Option<&'static str>,
    /// Upstream config documentation used when rendering config filenames in generated docs.
    pub config_doc_url: Option<&'static str>,
    /// Optional placements in generated overview tables.
    pub overviews: Vec<OverviewEntry>,
    /// Extended markdown documentation shown in the README detail section (behaviour, config examples).
    pub docs: &'static str,
}

impl Check {
    pub fn has_fix(&self) -> bool {
        match &self.kind {
            CheckKind::Template { fix_cmd, .. } => !fix_cmd.is_empty(),
            CheckKind::Native(native) => native.has_fix(),
        }
    }

    /// Returns false for checks implemented entirely in-process with no external binary.
    pub fn uses_binary(&self) -> bool {
        match &self.kind {
            CheckKind::Template { .. } => true,
            CheckKind::Native(native) => native.uses_binary(),
        }
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

    /// Returns the canonical mise.toml tool key to write when installing this
    /// check, or `None` if no mise entry is needed.
    pub fn install_key(&self) -> Option<&'static str> {
        if !self.uses_binary() || self.activate_unconditionally {
            return None;
        }
        Some(self.mise_tool_name.unwrap_or(self.bin_name))
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
            nonverbose_filter_prefixes: &[],
            stderr_filter_prefixes: &[],
            baseline_config: None,
            unsupported_configs: &[],
            allow_baseline_overlap_in_unsupported_configs: false,
            tool_key_migrations: vec![],
            check_type: None,
            adaptive_relevance: None,
            status_hook: None,
            nonverbose_failure_output: None,
            failure_output_patterns: &[],
            missing_component_hint: None,
            baseline_triggers: &[],
            is_formatter: false,
            ownership: Ownership::Generic,
            defers_to_formatters: false,
            editorconfig_line_length_policy: EditorconfigLineLengthPolicy::Default,
            activate_unconditionally: false,
            category: Category::Default,
            toolchain: None,
            kind: CheckKind::Template {
                check_cmd,
                fix_cmd: "",
                full_cmd: "",
                full_fix_cmd: "",
                scope,
            },
            java_jar: false,
            workflow_setup: None,
            fix_behavior: FixBehavior::Definitive,
            fix_order: None,
            desc: "",
            project_url: None,
            config_doc_url: None,
            overviews: vec![],
            docs: "",
        }
    }

    /// Native check with custom logic (not a simple command template).
    pub fn native(check_type: &'static dyn CheckType) -> Self {
        let Some(native) = check_type.native_check() else {
            panic!(
                "native check '{}' has no native check implementation",
                check_type.name()
            );
        };
        Check {
            name: check_type.name(),
            bin_name: native.bin_name().unwrap_or(""),
            mise_tool_name: None,
            version_range: None,
            patterns: &[],
            excludes_if_active: &[],
            linter_config: None,
            env: &[],
            nonverbose_filter_prefixes: &[],
            stderr_filter_prefixes: &[],
            baseline_config: None,
            unsupported_configs: &[],
            allow_baseline_overlap_in_unsupported_configs: false,
            tool_key_migrations: vec![],
            check_type: Some(check_type),
            adaptive_relevance: None,
            status_hook: None,
            nonverbose_failure_output: None,
            failure_output_patterns: &[],
            missing_component_hint: None,
            baseline_triggers: &[],
            is_formatter: false,
            ownership: Ownership::Generic,
            defers_to_formatters: false,
            editorconfig_line_length_policy: EditorconfigLineLengthPolicy::Default,
            activate_unconditionally: false,
            category: Category::Default,
            toolchain: None,
            java_jar: false,
            workflow_setup: None,
            fix_behavior: FixBehavior::Definitive,
            fix_order: None,
            kind: CheckKind::Native(NativeCheckRef::new(native)),
            desc: "",
            project_url: None,
            config_doc_url: None,
            overviews: vec![],
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

    /// Invoke this binary via `java -jar <path>` rather than directly.
    /// Use for self-executing JARs (e.g. ktlint and Checkstyle).
    pub fn java_jar(mut self) -> Self {
        self.java_jar = true;
        self
    }

    /// Mark as comprehensive-only in `flint init`. Pair with
    /// `.adaptive_relevance(...)` to skip on local default runs when irrelevant.
    pub fn slow(mut self) -> Self {
        self.category = Category::Slow;
        self
    }

    /// Mark as a formatter — files it owns are excluded from ec when both are active.
    pub fn formatter(mut self) -> Self {
        self.is_formatter = true;
        self.ownership = Ownership::Formatter;
        self
    }

    /// Mark this check as a domain-specific policy or security check.
    pub fn semantic(mut self) -> Self {
        self.ownership = Ownership::Semantic;
        self
    }

    /// Mark this check as a repository-wide/project check.
    pub fn project_ownership(mut self) -> Self {
        self.ownership = Ownership::Project;
        self
    }

    /// Set the explicit serial fix order for this check.
    pub fn fix_order(mut self, order: u16) -> Self {
        self.fix_order = Some(order);
        self
    }

    /// Skip files owned by active formatters (for ec — avoids double-checking).
    pub fn defer_to_formatters(mut self) -> Self {
        self.defers_to_formatters = true;
        self
    }

    /// Declare that this check owns `max_line_length` in `.editorconfig` for the
    /// given file patterns.
    pub fn editorconfig_line_length_off(
        mut self,
        patterns: &'static [&'static str],
        comment: &'static str,
        directive_style: Option<EditorconfigDirectiveStyle>,
    ) -> Self {
        self.editorconfig_line_length_policy = EditorconfigLineLengthPolicy::DisableForPatterns {
            patterns,
            comment,
            directive_style,
        };
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

    /// Set the upstream project page used for generated documentation links.
    pub fn project_url(mut self, project_url: &'static str) -> Self {
        self.project_url = Some(project_url);
        self
    }

    /// Set the upstream config documentation page used for generated config links.
    pub fn config_doc_url(mut self, config_doc_url: &'static str) -> Self {
        self.config_doc_url = Some(config_doc_url);
        self
    }

    /// Place this check in the generated overview tables.
    pub fn overview(
        mut self,
        section: OverviewSection,
        row_name: &'static str,
        role: OverviewRole,
        description: Option<&'static str>,
    ) -> Self {
        self.overviews.push(OverviewEntry {
            section,
            row_name,
            role,
            description,
        });
        self
    }

    /// Set extended markdown documentation shown in the README detail section.
    pub fn docs(mut self, docs: &'static str) -> Self {
        self.docs = docs;
        self
    }

    /// Override the patterns field (useful for native checks that need init-detection
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
        self.linter_config = Some(LinterConfig::File { file, flag });
        self
    }

    /// Inject `flag <config_dir>` when any of the named config files exist in
    /// `config_dir`. Useful for tools that accept a config directory instead of
    /// an individual config file path.
    pub fn linter_config_dir_if_any(
        mut self,
        files: &'static [&'static str],
        flag: &'static str,
    ) -> Self {
        self.linter_config = Some(LinterConfig::DirIfAny { files, flag });
        self
    }

    /// Set fixed environment variables when spawning this check's process.
    pub fn env(mut self, env: &'static [(&'static str, &'static str)]) -> Self {
        self.env = env;
        self
    }

    pub fn stderr_filter_prefixes(mut self, prefixes: &'static [&'static str]) -> Self {
        self.stderr_filter_prefixes = prefixes;
        self
    }

    pub fn nonverbose_filter_prefixes(mut self, prefixes: &'static [&'static str]) -> Self {
        self.nonverbose_filter_prefixes = prefixes;
        self
    }

    pub fn baseline_config(mut self, file: ConfigFile) -> Self {
        self.baseline_config = Some(file);
        self
    }

    pub fn unsupported_configs(mut self, files: &'static [ConfigFile]) -> Self {
        self.unsupported_configs = files;
        self
    }

    pub fn allow_baseline_overlap_in_unsupported_configs(mut self) -> Self {
        self.allow_baseline_overlap_in_unsupported_configs = true;
        self
    }

    pub fn check_type(mut self, check_type: &'static dyn CheckType) -> Self {
        self.check_type = Some(check_type);
        self
    }

    pub fn adaptive_relevance(mut self, hook: AdaptiveRelevanceHook) -> Self {
        self.adaptive_relevance = Some(hook);
        self
    }

    pub fn status_hook(mut self, hook: StatusHook) -> Self {
        self.status_hook = Some(hook);
        self
    }

    pub fn nonverbose_failure_output(mut self, hook: NonverboseFailureOutputHook) -> Self {
        self.nonverbose_failure_output = Some(hook);
        self
    }

    pub fn failure_output_patterns(mut self, patterns: &'static [&'static str]) -> Self {
        self.failure_output_patterns = patterns;
        self
    }

    pub fn missing_component_hint(
        mut self,
        component: &'static str,
        stderr_contains: &'static str,
    ) -> Self {
        self.missing_component_hint = Some(MissingComponentHint {
            component,
            stderr_contains,
        });
        self
    }

    pub fn baseline_triggers(mut self, files: &'static [ConfigFile]) -> Self {
        self.baseline_triggers = files;
        self
    }

    pub fn workflow_setup(mut self, setup: WorkflowSetup) -> Self {
        self.workflow_setup = Some(setup);
        self
    }

    /// Old mise tool keys that should always migrate to this check's current
    /// install key when encountered in `mise.toml`.
    pub fn migrate_tool_keys(mut self, old_keys: &'static [&'static str]) -> Self {
        self.tool_key_migrations
            .extend(old_keys.iter().map(|old_key| ToolKeyMigration { old_key }));
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
