use super::types::{
    Check, ConfigFile, EditorconfigDirectiveStyle, OverviewRole, OverviewSection, WorkflowSetup,
};
use crate::linters::{
    biome, flint_setup, license_header, lychee, renovate_deps,
    renovate_deps::RENOVATE_CONFIG_PATTERNS, rumdl, rustfmt, taplo, typos, yamllint,
};
const TOOL_RUMDL: &[&str] = &["tool", "rumdl"];
const TOOL_TYPOS: &[&str] = &["tool", "typos"];
const WORKSPACE_METADATA_TYPOS: &[&str] = &["workspace", "metadata", "typos"];
const PACKAGE_METADATA_TYPOS: &[&str] = &["package", "metadata", "typos"];
const TOOL_RUFF: &[&str] = &["tool", "ruff"];
const ACTIONLINT_URL: &str = "https://github.com/rhysd/actionlint";
const ACTIONLINT_CONFIG_URL: &str = "https://github.com/rhysd/actionlint/blob/main/docs/config.md";
const ZIZMOR_URL: &str = "https://github.com/zizmorcore/zizmor";
const ZIZMOR_CONFIG_URL: &str = "https://docs.zizmor.sh/configuration/";
const BIOME_URL: &str = "https://biomejs.dev/";
const BIOME_CONFIG_URL: &str = "https://biomejs.dev/guides/configure-biome/";
const CLIPPY_URL: &str = "https://doc.rust-lang.org/clippy/configuration.html";
const DOTNET_FORMAT_URL: &str = "https://learn.microsoft.com/dotnet/core/tools/dotnet-format";
const EDITORCONFIG_CHECKER_URL: &str =
    "https://github.com/editorconfig-checker/editorconfig-checker";
const EDITORCONFIG_CHECKER_CONFIG_URL: &str =
    "https://github.com/editorconfig-checker/editorconfig-checker?tab=readme-ov-file#configuration";
const GOFMT_URL: &str = "https://pkg.go.dev/cmd/gofmt";
const GOLANGCI_LINT_URL: &str = "https://golangci-lint.run/";
const GOLANGCI_LINT_CONFIG_URL: &str = "https://golangci-lint.run/usage/configuration/";
const GOOGLE_JAVA_FORMAT_URL: &str = "https://github.com/google/google-java-format";
const KUBE_LINTER_URL: &str = "https://github.com/stackrox/kube-linter";
const KUBE_LINTER_CONFIG_URL: &str = "https://docs.kubelinter.io/";
const HADOLINT_URL: &str = "https://github.com/hadolint/hadolint";
const HADOLINT_CONFIG_URL: &str =
    "https://github.com/hadolint/hadolint?tab=readme-ov-file#configure";
const KTLINT_URL: &str = "https://github.com/ktlint/ktlint";
const KTLINT_CONFIG_URL: &str =
    "https://pinterest.github.io/ktlint/latest/rules/configuration-ktlint/";
const LYCHEE_URL: &str = "https://lychee.cli.rs/";
const RENOVATE_URL: &str = "https://docs.renovatebot.com/";
const RUFF_URL: &str = "https://docs.astral.sh/ruff/";
const RUFF_CONFIG_URL: &str = "https://docs.astral.sh/ruff/configuration/";
const RUMDL_URL: &str = "https://rumdl.dev/";
const RUMDL_CONFIG_URL: &str = "https://rumdl.dev/mdformat-comparison/#configuration";
const RUSTFMT_URL: &str = "https://github.com/rust-lang/rustfmt";
const RUSTFMT_CONFIG_URL: &str =
    "https://github.com/rust-lang/rustfmt?tab=readme-ov-file#configuring-rustfmt";
const SHELLCHECK_URL: &str = "https://github.com/koalaman/shellcheck";
const SHELLCHECK_CONFIG_URL: &str =
    "https://github.com/koalaman/shellcheck/blob/master/shellcheck.1.md";
const SHFMT_URL: &str = "https://github.com/mvdan/sh";
const TAPLO_URL: &str = "https://taplo.tamasfe.dev/";
const TAPLO_CONFIG_URL: &str = "https://taplo.tamasfe.dev/configuration/file.html";
const TYPOS_URL: &str = "https://github.com/crate-ci/typos";
const TYPOS_CONFIG_URL: &str = "https://github.com/crate-ci/typos/blob/master/docs/reference.md";
const XMLLINT_URL: &str = "https://github.com/jonwiggins/xmloxide";
const YAMLLINT_CONFIG_URL: &str = "https://yamllint.readthedocs.io/en/stable/configuration.html";
const RYL_URL: &str = "https://github.com/owenlamont/ryl";

const KUBE_LINTER_PATTERNS: &[&str] = &[
    "k8s/*.yml",
    "k8s/*.yaml",
    "kubernetes/*.yml",
    "kubernetes/*.yaml",
    "manifests/*.yml",
    "manifests/*.yaml",
];

const SHELLCHECK_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("shellcheckrc"),
    ConfigFile::project("shellcheckrc"),
];
const RUMDL_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("rumdl.toml"),
    ConfigFile::project("rumdl.toml"),
    ConfigFile::project(".config/rumdl.toml"),
    ConfigFile::project_toml_section("pyproject.toml", TOOL_RUMDL),
];
const YAMLLINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".yamllint"),
    ConfigFile::config_dir(".yamllint.yaml"),
    ConfigFile::project(".yamllint"),
    ConfigFile::project(".yamllint.yaml"),
];
const TAPLO_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("taplo.toml"),
    ConfigFile::project(".taplo.toml"),
    ConfigFile::project("taplo.toml"),
];
const ACTIONLINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("actionlint.yaml"),
    ConfigFile::project(".github/actionlint.yaml"),
    ConfigFile::project(".github/actionlint.yml"),
];
const ZIZMOR_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("zizmor.yaml"),
    ConfigFile::project("zizmor.yml"),
    ConfigFile::project("zizmor.yaml"),
    ConfigFile::project(".github/zizmor.yml"),
    ConfigFile::project(".github/zizmor.yaml"),
];
const HADOLINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".hadolint.yml"),
    ConfigFile::project(".hadolint.yml"),
];
const TYPOS_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("typos.toml"),
    ConfigFile::config_dir(".typos.toml"),
    ConfigFile::project(".codespellrc"),
    ConfigFile::project("typos.toml"),
    ConfigFile::project(".typos.toml"),
    ConfigFile::project_toml_section("pyproject.toml", TOOL_TYPOS),
    ConfigFile::project_toml_section("Cargo.toml", WORKSPACE_METADATA_TYPOS),
    ConfigFile::project_toml_section("Cargo.toml", PACKAGE_METADATA_TYPOS),
];
const EDITORCONFIG_CHECKER_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".ecrc"),
    ConfigFile::project(".ecrc"),
];
const EDITORCONFIG_CHECKER_BASELINE_TRIGGERS: &[ConfigFile] =
    &[ConfigFile::project(".editorconfig")];
const GOLANGCI_LINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".golangci.yaml"),
    ConfigFile::config_dir(".golangci.toml"),
    ConfigFile::config_dir(".golangci.json"),
    ConfigFile::project(".golangci.yaml"),
    ConfigFile::project(".golangci.toml"),
    ConfigFile::project(".golangci.json"),
];
const BIOME_BASELINE_CONFIG: ConfigFile = ConfigFile::project("biome.jsonc");
const BIOME_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[ConfigFile::project("biome.json")];
const RUSTFMT_BASELINE_CONFIG: ConfigFile = ConfigFile::config_dir("rustfmt.toml");
const RUSTFMT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::project("rustfmt.toml"),
    ConfigFile::project(".rustfmt.toml"),
];
const RUFF_BASELINE_CONFIG: ConfigFile = ConfigFile::config_dir("ruff.toml");
const RUFF_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".ruff.toml"),
    ConfigFile::project(".ruff.toml"),
    ConfigFile::project_toml_section("pyproject.toml", TOOL_RUFF),
];

/// Built-in linter registry.
///
/// # Naming convention
///
/// Prefer the user-facing binary or native command name:
/// - `shellcheck` → `shellcheck`
/// - `aqua:owenlamont/ryl` → `ryl`
/// - `aqua:jonwiggins/xmloxide` → `xmllint`
///
/// Exceptions are explicit and should stay rare:
/// - clearer package-facing names such as `editorconfig-checker` over `ec`
/// - native subcommands such as `cargo-fmt`, `ruff-format`, and `dotnet-format`
fn check_shellcheck() -> Check {
    Check::file(
        "shellcheck",
        "shellcheck -x -P SCRIPTDIR {FILE}",
        &["*.sh", "*.bash", "*.bats"],
    )
    .linter_config(".shellcheckrc", "--rcfile")
    .baseline_config(ConfigFile::config_dir(".shellcheckrc"))
    .unsupported_configs(SHELLCHECK_UNSUPPORTED_CONFIGS)
    .project_url(SHELLCHECK_URL)
    .config_doc_url(SHELLCHECK_CONFIG_URL)
    .overview(
        OverviewSection::FilesFormats,
        "Shell",
        OverviewRole::Linter,
        None,
    )
    .migrate_tool_keys(&["github:koalaman/shellcheck"])
    .desc("Lint shell scripts for common mistakes")
    .style()
}

fn check_shfmt() -> Check {
    Check::file("shfmt", "shfmt -d {FILE}", &["*.sh", "*.bash"])
        .fix("shfmt -w {FILE}")
        .project_url(SHFMT_URL)
        .overview(
            OverviewSection::FilesFormats,
            "Shell",
            OverviewRole::Formatter,
            None,
        )
        .formatter()
        .migrate_tool_keys(&["github:mvdan/sh"])
        .desc("Format shell scripts")
        .style()
}

fn check_rumdl() -> Check {
    Check::files("rumdl", "rumdl check {FILES}", &["*.md"])
        .fix("rumdl check --fix {FILES}")
        .linter_config(".rumdl.toml", "--config")
        .baseline_config(ConfigFile::config_dir(".rumdl.toml"))
        .unsupported_configs(RUMDL_UNSUPPORTED_CONFIGS)
        .project_url(RUMDL_URL)
        .config_doc_url(RUMDL_CONFIG_URL)
        .overview(
            OverviewSection::FilesFormats,
            "Markdown",
            OverviewRole::Both,
            None,
        )
        .check_type(&rumdl::CHECK_TYPE)
        .nonverbose_filter_prefixes(&["Success: No issues found in "])
        .formatter()
        .editorconfig_line_length_off(
            &["*.md"],
            "Markdown line length is handled by rumdl",
            Some(EditorconfigDirectiveStyle::Html),
        )
        .desc("Lint Markdown files for style and consistency")
}

fn check_yaml_lint() -> Check {
    Check::files("ryl", "ryl {FILES}", &["*.yml", "*.yaml"])
        .fix("ryl --fix {FILES}")
        .linter_config(".yamllint.yml", "-c")
        .baseline_config(ConfigFile::config_dir(".yamllint.yml"))
        .unsupported_configs(YAMLLINT_UNSUPPORTED_CONFIGS)
        .project_url(RYL_URL)
        .config_doc_url(YAMLLINT_CONFIG_URL)
        .overview(
            OverviewSection::FilesFormats,
            "YAML",
            OverviewRole::Both,
            None,
        )
        .check_type(&yamllint::CHECK_TYPE)
        .formatter()
        .desc("Lint YAML files for style and consistency")
        .mise_tool("aqua:owenlamont/ryl")
        .migrate_tool_keys(&["cargo:yaml-lint", "github:owenlamont/ryl"])
}

fn check_kube_linter() -> Check {
    Check::native(&crate::linters::kube_linter::CHECK_TYPE)
        .patterns(KUBE_LINTER_PATTERNS)
        .mise_tool("aqua:stackrox/kube-linter")
        .baseline_config(ConfigFile::config_dir("kube-linter.yaml"))
        .project_url(KUBE_LINTER_URL)
        .config_doc_url(KUBE_LINTER_CONFIG_URL)
        .overview(
            OverviewSection::ToolingCi,
            "Kubernetes manifests",
            OverviewRole::Check,
            Some("Kubernetes security and production-readiness policy"),
        )
        .desc("Lint explicitly selected Kubernetes resources")
        .docs(
            "KubeLinter is report-only and only runs on files selected by\
            \n\
            [checks.kube-linter].paths (or existing conventional k8s/,\
            \n\
            kubernetes/, or manifests/ directories). Flint parses YAML documents\
            \n\
            and passes only documents containing apiVersion and kind, so ordinary\
            \n\
            YAML and Docker Compose files are not treated as Kubernetes resources.\
            \n\
            Helm/Kustomize rendering remains an explicit separate workflow.",
        )
        .style()
}

fn check_taplo() -> Check {
    Check::file(
        "taplo",
        "taplo fmt {CONFIG_ARGS} --check {FILE}",
        &["*.toml"],
    )
    .fix("taplo fmt {CONFIG_ARGS} {FILE}")
    .linter_config(".taplo.toml", "--config")
    .baseline_config(ConfigFile::config_dir(".taplo.toml"))
    .unsupported_configs(TAPLO_UNSUPPORTED_CONFIGS)
    .project_url(TAPLO_URL)
    .config_doc_url(TAPLO_CONFIG_URL)
    .overview(
        OverviewSection::FilesFormats,
        "TOML",
        OverviewRole::Formatter,
        None,
    )
    .check_type(&taplo::CHECK_TYPE)
    .stderr_filter_prefixes(&[" INFO taplo:"])
    .nonverbose_failure_output(taplo::normalize_nonverbose_failure_output)
    .formatter()
    .migrate_tool_keys(&["github:tamasfe/taplo"])
    .desc("Format TOML files")
    .docs(
        "Formats TOML files with [Taplo](https://taplo.tamasfe.dev/).\n\
            \n\
            This check intentionally stays basic: it uses `taplo fmt --check` for\n\
            verification and `taplo fmt` for `--fix`. That keeps behavior aligned with\n\
            flint's existing formatter-style checks.\n\
            \n\
            Current caveat: Taplo's published docs currently advertise TOML 1.0.0\n\
            support, so treat this check as TOML 1.0-oriented for now.",
    )
    .style()
}

fn check_actionlint() -> Check {
    Check::file(
        "actionlint",
        "actionlint {FILE}",
        &[".github/workflows/*.yml", ".github/workflows/*.yaml"],
    )
    .linter_config("actionlint.yml", "-config-file")
    .baseline_config(ConfigFile::config_dir("actionlint.yml"))
    .unsupported_configs(ACTIONLINT_UNSUPPORTED_CONFIGS)
    .project_url(ACTIONLINT_URL)
    .config_doc_url(ACTIONLINT_CONFIG_URL)
    .overview(
        OverviewSection::ToolingCi,
        "GitHub Actions",
        OverviewRole::Check,
        None,
    )
    .desc("Lint GitHub Actions workflow files")
    .style()
}

fn check_zizmor() -> Check {
    Check::files(
        "zizmor",
        "zizmor {FILES}",
        &[".github/workflows/*.yml", ".github/workflows/*.yaml"],
    )
    .fix("zizmor --fix {FILES}")
    .linter_config("zizmor.yml", "--config")
    .baseline_config(ConfigFile::config_dir("zizmor.yml"))
    .unsupported_configs(ZIZMOR_UNSUPPORTED_CONFIGS)
    .allow_baseline_overlap_in_unsupported_configs()
    .nonverbose_filter_prefixes(&[
        "No findings to report. Good job!",
        "No fixes available to apply.",
        " INFO zizmor:",
        " INFO audit: zizmor:",
    ])
    .project_url(ZIZMOR_URL)
    .config_doc_url(ZIZMOR_CONFIG_URL)
    .overview(
        OverviewSection::ToolingCi,
        "GitHub Actions",
        OverviewRole::Check,
        None,
    )
    .desc("Audit GitHub Actions workflows for security issues")
    .docs(
        "zizmor can drift without file changes: its `ref-version-mismatch`\n\
        audit resolves pinned action hashes against GitHub's tag API at\n\
        run-time. When a maintainer moves a mutable tag (e.g. `v6` advances\n\
        to a new patch), workflows pinned to the old commit but commented\n\
        `# v6` become inconsistent without any local file change. Flint\n\
        scans only files changed in the PR, so drift in untouched workflows\n\
        stays invisible until something edits them. Run `flint run --full`\n\
        periodically (e.g. weekly `schedule:` workflow) to catch this.",
    )
    .style()
}

fn check_hadolint() -> Check {
    Check::file(
        "hadolint",
        "hadolint {FILE}",
        &["Dockerfile", "Dockerfile.*", "*.dockerfile"],
    )
    .linter_config(".hadolint.yaml", "--config")
    .baseline_config(ConfigFile::config_dir(".hadolint.yaml"))
    .unsupported_configs(HADOLINT_UNSUPPORTED_CONFIGS)
    .project_url(HADOLINT_URL)
    .config_doc_url(HADOLINT_CONFIG_URL)
    .overview(
        OverviewSection::ToolingCi,
        "Dockerfile",
        OverviewRole::Check,
        None,
    )
    .desc("Lint Dockerfiles")
    .style()
}

fn check_xmllint() -> Check {
    Check::files("xmllint", "xmllint --noout {FILES}", &["*.xml"])
        .mise_tool("aqua:jonwiggins/xmloxide")
        .project_url(XMLLINT_URL)
        .overview(
            OverviewSection::FilesFormats,
            "XML",
            OverviewRole::Linter,
            None,
        )
        .migrate_tool_keys(&["cargo:xmloxide", "github:jonwiggins/xmloxide"])
        .desc("Validate XML files are well-formed")
}

fn check_typos() -> Check {
    Check::files("typos", "typos --force-exclude {FILES}", &["*"])
        .fix("typos --write-changes --force-exclude {FILES}")
        .linter_config("_typos.toml", "--config")
        .baseline_config(ConfigFile::config_dir("_typos.toml"))
        .unsupported_configs(TYPOS_UNSUPPORTED_CONFIGS)
        .allow_baseline_overlap_in_unsupported_configs()
        .project_url(TYPOS_URL)
        .config_doc_url(TYPOS_CONFIG_URL)
        .overview(
            OverviewSection::General,
            "Spelling",
            OverviewRole::Check,
            Some("Spelling in source and text files"),
        )
        .check_type(&typos::CHECK_TYPE)
        .migrate_tool_keys(&["codespell", "pipx:codespell", "aqua:crate-ci/typos"])
        .desc("Check for common spelling mistakes")
        .mise_tool("typos")
}

fn check_editorconfig_checker() -> Check {
    // Defer to formatters that enforce line length — those are the ones
    // that conflict with ec's max_line_length editorconfig check.
    // Note: ec's -config flag controls ec's own JSON config, not .editorconfig itself.
    Check::files("editorconfig-checker", "ec {FILES}", &["*"])
        .bin("ec")
        .mise_tool("editorconfig-checker")
        .defer_to_formatters()
        .linter_config(".editorconfig-checker.json", "-config")
        .baseline_triggers(EDITORCONFIG_CHECKER_BASELINE_TRIGGERS)
        .unsupported_configs(EDITORCONFIG_CHECKER_UNSUPPORTED_CONFIGS)
        .project_url(EDITORCONFIG_CHECKER_URL)
        .config_doc_url(EDITORCONFIG_CHECKER_CONFIG_URL)
        .overview(
            OverviewSection::General,
            "EditorConfig",
            OverviewRole::Check,
            Some("EditorConfig compliance"),
        )
        .desc("Check files comply with EditorConfig settings")
        .docs(
            "`editorconfig-checker` defers to formatters: it runs on all files\n\
            but automatically skips file types owned by an active formatter. If\n\
            none of those formatters are installed, `editorconfig-checker` checks\n\
            those files itself.\n\
            \n\
            Flint writes shared `.editorconfig` carve-outs for known\n\
            formatter-owned line length: today that means `rumdl` for `*.md`,\n\
            `rustfmt` for `*.rs`, and `google-java-format` for `*.java`. Those\n\
            sections use `max_line_length = off` so editors and\n\
            `editorconfig-checker` share the same intent instead of relying on\n\
            checker-specific JSON excludes. If a matching section already\n\
            exists, `flint init` rewrites its `max_line_length` to `off`\n\
            instead of leaving a formatter-conflicting numeric value in place.",
        )
}

fn check_golangci_lint() -> Check {
    Check::project(
        "golangci-lint",
        "golangci-lint run --new-from-rev={MERGE_BASE}",
        &["*.go"],
    )
    .linter_config(".golangci.yml", "--config")
    .baseline_config(ConfigFile::config_dir(".golangci.yml"))
    .unsupported_configs(GOLANGCI_LINT_UNSUPPORTED_CONFIGS)
    .project_url(GOLANGCI_LINT_URL)
    .config_doc_url(GOLANGCI_LINT_CONFIG_URL)
    .overview(OverviewSection::Languages, "Go", OverviewRole::Linter, None)
    .desc("Lint Go code; uses --new-from-rev to scope analysis to changed code")
    .lang()
}

fn check_ruff() -> Check {
    Check::file("ruff", "ruff check {FILE}", &["*.py"])
        .fix("ruff check --fix {FILE}")
        .linter_config("ruff.toml", "--config")
        .baseline_config(RUFF_BASELINE_CONFIG)
        .unsupported_configs(RUFF_UNSUPPORTED_CONFIGS)
        .project_url(RUFF_URL)
        .config_doc_url(RUFF_CONFIG_URL)
        .overview(
            OverviewSection::Languages,
            "Python",
            OverviewRole::Linter,
            None,
        )
        .migrate_tool_keys(&["pipx:ruff", "github:astral-sh/ruff"])
        .desc("Lint Python code")
        .lang()
}

fn check_ruff_format() -> Check {
    Check::file("ruff-format", "ruff format --check {FILE}", &["*.py"])
        .bin("ruff")
        .fix("ruff format {FILE}")
        .fix_after("ruff")
        .linter_config("ruff.toml", "--config")
        .baseline_config(RUFF_BASELINE_CONFIG)
        .unsupported_configs(RUFF_UNSUPPORTED_CONFIGS)
        .project_url(RUFF_URL)
        .config_doc_url(RUFF_CONFIG_URL)
        .overview(
            OverviewSection::Languages,
            "Python",
            OverviewRole::Formatter,
            None,
        )
        .formatter()
        .desc("Format Python code")
        .mise_tool("ruff")
        .lang()
}

fn check_biome() -> Check {
    Check::file(
        "biome",
        "biome check {FILE}",
        &["*.json", "*.jsonc", "*.js", "*.ts", "*.jsx", "*.tsx"],
    )
    .fix("biome check --fix {FILE}")
    .baseline_config(BIOME_BASELINE_CONFIG)
    .unsupported_configs(BIOME_UNSUPPORTED_CONFIGS)
    .project_url(BIOME_URL)
    .config_doc_url(BIOME_CONFIG_URL)
    .overview(
        OverviewSection::Languages,
        "JavaScript / TypeScript",
        OverviewRole::Linter,
        None,
    )
    .overview(
        OverviewSection::FilesFormats,
        "JSON",
        OverviewRole::Linter,
        None,
    )
    .check_type(&biome::CHECK_TYPE)
    .migrate_tool_keys(&["npm:@biomejs/biome"])
    .desc("Lint JS/TS/JSON files")
    .lang()
}

fn check_biome_format() -> Check {
    Check::file(
        "biome-format",
        "biome format {FILE}",
        &["*.json", "*.jsonc", "*.js", "*.ts", "*.jsx", "*.tsx"],
    )
    .bin("biome")
    .fix("biome format --write {FILE}")
    .fix_after("biome")
    .baseline_config(BIOME_BASELINE_CONFIG)
    .unsupported_configs(BIOME_UNSUPPORTED_CONFIGS)
    .project_url(BIOME_URL)
    .config_doc_url(BIOME_CONFIG_URL)
    .overview(
        OverviewSection::Languages,
        "JavaScript / TypeScript",
        OverviewRole::Formatter,
        None,
    )
    .overview(
        OverviewSection::FilesFormats,
        "JSON",
        OverviewRole::Formatter,
        None,
    )
    .check_type(&biome::CHECK_TYPE)
    .formatter()
    .desc("Format JS/TS/JSON files")
    .mise_tool("biome")
    .lang()
}

fn check_cargo_clippy() -> Check {
    Check::project(
        "cargo-clippy",
        "cargo clippy -q --all-targets -- -D warnings",
        &["*.rs"],
    )
    .fix("cargo clippy -q --all-targets --fix --allow-dirty --allow-staged -- -D warnings")
    .partial_fix()
    .mise_tool("rust")
    .toolchain_components("clippy")
    .missing_component_hint(
        "clippy",
        "'cargo-clippy' is not installed for the toolchain",
    )
    .workflow_setup(WorkflowSetup::RustComponents)
    .project_url(CLIPPY_URL)
    .overview(
        OverviewSection::Languages,
        "Rust",
        OverviewRole::Linter,
        None,
    )
    .desc("Lint Rust code; runs on all .rs files, not just changed")
    .lang()
}

fn check_cargo_fmt() -> Check {
    Check::project("cargo-fmt", "cargo fmt -- {CONFIG_ARGS} --check", &["*.rs"])
        .fix("cargo fmt -- {CONFIG_ARGS}")
        .fix_after("cargo-clippy")
        .linter_config("rustfmt.toml", "--config-path")
        .baseline_config(RUSTFMT_BASELINE_CONFIG)
        .unsupported_configs(RUSTFMT_UNSUPPORTED_CONFIGS)
        .project_url(RUSTFMT_URL)
        .config_doc_url(RUSTFMT_CONFIG_URL)
        .overview(
            OverviewSection::Languages,
            "Rust",
            OverviewRole::Formatter,
            None,
        )
        .check_type(&rustfmt::CHECK_TYPE)
        .bin("rustfmt")
        .mise_tool("rust")
        .toolchain_components("rustfmt")
        .missing_component_hint("rustfmt", "'rustfmt' is not installed for the toolchain")
        .workflow_setup(WorkflowSetup::RustComponents)
        .formatter()
        .editorconfig_line_length_off(&["*.rs"], "Rust line length is handled by rustfmt", None)
        .desc("Format Rust code; runs on all .rs files, not just changed")
        .lang()
}

fn check_gofmt() -> Check {
    Check::file("gofmt", "gofmt -d {FILE}", &["*.go"])
        .fix("gofmt -w {FILE}")
        .mise_tool("go")
        .toolchain()
        .project_url(GOFMT_URL)
        .overview(
            OverviewSection::Languages,
            "Go",
            OverviewRole::Formatter,
            None,
        )
        .formatter()
        .desc("Format Go code")
        .lang()
}

fn check_google_java_format() -> Check {
    Check::files(
        "google-java-format",
        "google-java-format --dry-run --set-exit-if-changed {FILES}",
        &["*.java"],
    )
    .fix("google-java-format -i {FILES}")
    .mise_tool("google-java-format")
    .formatter()
    .editorconfig_line_length_off(
        &["*.java"],
        "Java line length is handled by google-java-format",
        Some(EditorconfigDirectiveStyle::Slash),
    )
    .migrate_tool_keys(&[
        "ubi:google/google-java-format",
        "github:google/google-java-format",
    ])
    .project_url(GOOGLE_JAVA_FORMAT_URL)
    .overview(
        OverviewSection::Languages,
        "Java",
        OverviewRole::Formatter,
        None,
    )
    .desc("Format Java code")
    .lang()
}

fn check_ktlint() -> Check {
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
    .windows_java_jar()
    .formatter()
    .project_url(KTLINT_URL)
    .config_doc_url(KTLINT_CONFIG_URL)
    .overview(
        OverviewSection::Languages,
        "Kotlin",
        OverviewRole::Both,
        None,
    )
    .migrate_tool_keys(&["ubi:pinterest/ktlint"])
    .migrate_tool_keys(&["github:pinterest/ktlint"])
    .desc("Lint and format Kotlin code")
    .lang()
}

fn check_dotnet_format() -> Check {
    Check::files(
        "dotnet-format",
        "dotnet format --verify-no-changes --include {RELFILES}",
        &["*.cs"],
    )
    .fix("dotnet format --include {RELFILES}")
    .full_cmd("dotnet format --verify-no-changes", "dotnet format")
    .bin("dotnet")
    .mise_tool("dotnet")
    .toolchain()
    .project_url(DOTNET_FORMAT_URL)
    .overview(
        OverviewSection::Languages,
        "C#",
        OverviewRole::Formatter,
        None,
    )
    .formatter()
    .desc("Format C# code")
    .lang()
}

fn check_lychee() -> Check {
    Check::native(&lychee::CHECK_TYPE)
        .project_url(LYCHEE_URL)
        .overview(
            OverviewSection::General,
            "Links",
            OverviewRole::Check,
            Some("Broken links"),
        )
        .desc("Check for broken links")
        .docs(
            "Orchestrates [lychee](https://lychee.cli.rs/) for link checking. \
            Requires `lychee` in `[tools]`.\n\
            \n\
            Default behavior: checks all links in changed files. In CI, Flint also adds a\n\
            full-repository safeguard pass over local links in all files so broken internal\n\
            links in unchanged docs still fail the build. Outside that CI safeguard, setting\n\
            `check_all_local = true` in `flint.toml` adds the same local-links-only pass\n\
            over all files.\n\
            \n\
            Outside CI, flint also enables a local lychee request cache by default to\n\
            speed up repeated runs. Flint stores that cache under `.lychee_cache/` and\n\
            creates the directory on first use. Set `FLINT_LYCHEE_SKIP_LOCAL_CACHE=true`\n\
            to opt out. If your lychee config already sets `cache = true`, flint leaves\n\
            caching to lychee instead.\n\
            \n\
            In CI, `lychee` requires `GITHUB_TOKEN` so GitHub link checks can authenticate.\n\
            On GitHub Actions PR runs in changed-file mode, link remaps also require\n\
            `GITHUB_REPOSITORY`, `GITHUB_BASE_REF`, `GITHUB_HEAD_REF`, and `PR_HEAD_REPO`.\n\
            GitHub Actions provides the first three; set `PR_HEAD_REPO` from\n\
            `github.event.pull_request.head.repo.full_name`. The CI local-links safeguard\n\
            pass and `--full` do not require the PR remap metadata.\n\
            \n\
            Configure via `flint.toml`:\n\
            \n\
            ```toml\n\
            [checks.links]\n\
            config = \".github/config/lychee.toml\"\n\
            check_all_local = true\n\
            ```",
        )
}

fn check_renovate_deps() -> Check {
    Check::native(&renovate_deps::CHECK_TYPE)
        .slow()
        .adaptive_relevance(renovate_deps::adaptive_relevance)
        .mise_tool("npm:renovate")
        .patterns(RENOVATE_CONFIG_PATTERNS)
        .project_url(RENOVATE_URL)
        .overview(
            OverviewSection::General,
            "Renovate",
            OverviewRole::Check,
            Some("Dependency update configuration"),
        )
        .desc("Verify Renovate dependency snapshot is up to date")
        .docs(
            "Verifies `renovate-tracked-deps.json` next to the active Renovate\n\
            config is up to date by running Renovate locally and comparing its\n\
            output against the committed snapshot.\n\
            It also checks that dependencies extracted from different files but\n\
            resolving to the same upstream package match the same Renovate\n\
            package rules. That catches config splits like `actionlint` vs\n\
            `rhysd/actionlint` before Renovate stops grouping them consistently.\n\
            Requires `renovate` in `[tools]`.\n\
            \n\
            In CI, `renovate-deps` requires `GITHUB_COM_TOKEN` or `GITHUB_TOKEN`\n\
            so Renovate can authenticate GitHub requests. If `GITHUB_COM_TOKEN` is\n\
            unset, flint forwards `GITHUB_TOKEN` to Renovate as `GITHUB_COM_TOKEN`.\n\
            \n\
            When `flint init` writes a new `flint.toml`, it includes this section if\n\
            `renovate-deps` is selected.\n\
            \n\
            With `--fix`, automatically regenerates and commits the snapshot.\n\
            For custom/regex managers, prefer canonical `depNameTemplate` values\n\
            for grouping and explicit `packageNameTemplate` values for datasource\n\
            lookups when those identities differ.\n\
            See [the renovate-deps guide](linters/renovate-deps.md) for examples.\n\
            \n\
            Configure via `flint.toml`:\n\
            \n\
            ```toml\n\
            [checks.renovate-deps]\n\
            exclude_managers = [\"github-actions\", \"github-runners\"]\n\
            ```",
        )
}

fn check_license_header() -> Check {
    Check::native(&license_header::CHECK_TYPE)
        .activate_unconditionally()
        .status_hook(license_header::status)
        .overview(
            OverviewSection::General,
            "License headers",
            OverviewRole::Check,
            Some("Required file header text"),
        )
        .desc("Check source files have the required license header")
        .docs(
            "Disabled by default. Configure in `flint.toml`:\n\
            \n\
            ```toml\n\
            [checks.license-header]\n\
            text = \"SPDX-License-Identifier: Apache-2.0\"\n\
            patterns = [\"*.java\", \"*.kt\"]\n\
            lines_to_check = 5\n\
            ```\n\
            \n\
            - `text` — required header text to find near the top of each file\n\
            - `patterns` — glob patterns selecting which files to check\n\
            - `lines_to_check` — how many leading lines to search; defaults to `5`\n\
            \n\
            `text` may be multi-line. Flint joins the first `lines_to_check` lines with\n\
            newlines and checks whether that text contains the configured header snippet.",
        )
}

fn check_flint_setup() -> Check {
    Check::native(&flint_setup::CHECK_TYPE)
        .activate_unconditionally()
        .patterns(&["mise.toml"])
        .overview(
            OverviewSection::General,
            "Flint setup",
            OverviewRole::Check,
            Some("Flint-managed setup and `mise.toml` layout"),
        )
        .desc("Keep Flint setup current and mise.toml lint tooling canonical")
        .docs(
            "Checks the repo's Flint-managed setup state and `mise.toml` layout.\n\
            \n\
            This verifies and fixes Flint-managed setup:\n\
            - apply versioned Flint setup migrations\n\
            - replace obsolete lint tool keys with their supported successors\n\
            - reject unsupported legacy lint tools that need repo migrations\n\
            - sort `[tools]` entries into Flint's canonical order\n\
            - keep lint-managed tool entries under the `# Linters` header\n\
            - keep runtime, SDK, and unknown tool entries above that header\n\
            \n\
            With `--fix`, rewrites Flint-managed config in place and applies any\n\
            currently actionable setup migration.",
        )
}

pub fn builtin() -> Vec<Check> {
    vec![
        check_flint_setup(),
        check_shellcheck(),
        check_shfmt(),
        check_rumdl(),
        check_yaml_lint(),
        check_kube_linter(),
        check_taplo(),
        check_actionlint(),
        check_zizmor(),
        check_hadolint(),
        check_xmllint(),
        check_typos(),
        check_editorconfig_checker(),
        check_golangci_lint(),
        check_ruff(),
        check_ruff_format(),
        check_biome(),
        check_biome_format(),
        check_cargo_clippy(),
        check_cargo_fmt(),
        check_gofmt(),
        check_google_java_format(),
        check_ktlint(),
        check_dotnet_format(),
        check_lychee(),
        check_renovate_deps(),
        check_license_header(),
    ]
}
