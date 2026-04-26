use super::types::{Check, ConfigFile, EditorconfigDirectiveStyle, SpecialKind};
use crate::linters::renovate_deps::RENOVATE_CONFIG_PATTERNS;
use crate::setup::{V1_BOOTSTRAP_SETUP_VERSION, V2_BASELINE_SETUP_VERSION};

const TOOL_RUMDL: &[&str] = &["tool", "rumdl"];
const TOOL_CODESPELL: &[&str] = &["tool", "codespell"];
const TOOL_RUFF: &[&str] = &["tool", "ruff"];

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
const HADOLINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".hadolint.yml"),
    ConfigFile::project(".hadolint.yml"),
];
const CODESPELL_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::project_ini_section("setup.cfg", "codespell"),
    ConfigFile::project_toml_section("pyproject.toml", TOOL_CODESPELL),
];
const EDITORCONFIG_CHECKER_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".ecrc"),
    ConfigFile::project(".ecrc"),
];
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
/// A check's `name` is the last path segment of its mise tool key (after `:` or `/`):
/// - `editorconfig-checker` → name `editorconfig-checker` (not the binary `ec`)
/// - `aqua:owenlamont/ryl` → name `ryl`
/// - `ktlint` → name `ktlint`
///
/// Exceptions:
/// - formatter variants may use a `-fmt` suffix (e.g. `ruff-fmt`)
/// - language toolchains shared across multiple binaries use the command name instead
///   (e.g. `cargo-fmt`, `cargo-clippy`) because `rust` would be ambiguous
fn check_shellcheck() -> Check {
    Check::file(
        "shellcheck",
        "shellcheck -x -P SCRIPTDIR {FILE}",
        &["*.sh", "*.bash", "*.bats"],
    )
    .mise_tool("github:koalaman/shellcheck")
    .linter_config(".shellcheckrc", "--rcfile")
    .baseline_config(ConfigFile::config_dir(".shellcheckrc"))
    .unsupported_configs(SHELLCHECK_UNSUPPORTED_CONFIGS)
    .migrate_tool_keys_after(V2_BASELINE_SETUP_VERSION, &["shellcheck"])
    .desc("Lint shell scripts for common mistakes")
    .style()
}

fn check_shfmt() -> Check {
    Check::file("shfmt", "shfmt -d {FILE}", &["*.sh", "*.bash"])
        .fix("shfmt -w {FILE}")
        .formatter()
        .migrate_tool_keys_after(V1_BOOTSTRAP_SETUP_VERSION, &["github:mvdan/sh"])
        .desc("Format shell scripts")
        .style()
}

fn check_rumdl() -> Check {
    Check::file("rumdl", "rumdl check {FILE}", &["*.md"])
        .fix("rumdl check --fix {FILE}")
        .linter_config(".rumdl.toml", "--config")
        .baseline_config(ConfigFile::config_dir(".rumdl.toml"))
        .unsupported_configs(RUMDL_UNSUPPORTED_CONFIGS)
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
        .formatter()
        .desc("Lint YAML files for style and consistency")
        .mise_tool("aqua:owenlamont/ryl")
        .migrate_tool_keys_after(
            V2_BASELINE_SETUP_VERSION,
            &["cargo:yaml-lint", "github:owenlamont/ryl"],
        )
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
    .stderr_filter_prefixes(&[" INFO taplo:"])
    .formatter()
    .migrate_tool_keys_after(V2_BASELINE_SETUP_VERSION, &["github:tamasfe/taplo"])
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
    .desc("Lint GitHub Actions workflow files")
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
    .desc("Lint Dockerfiles")
    .style()
}

fn check_xmllint() -> Check {
    Check::files("xmloxide", "xmllint --noout {FILES}", &["*.xml"])
        .bin("xmllint")
        .mise_tool("github:jonwiggins/xmloxide")
        .migrate_tool_keys_after(V2_BASELINE_SETUP_VERSION, &["cargo:xmloxide"])
        .desc("Validate XML files are well-formed")
}

fn check_codespell() -> Check {
    Check::files("codespell", "codespell {FILES}", &["*"])
        .fix("codespell --write-changes {FILES}")
        .linter_config(".codespellrc", "--config")
        .baseline_config(ConfigFile::config_dir(".codespellrc"))
        .unsupported_configs(CODESPELL_UNSUPPORTED_CONFIGS)
        .desc("Check for common spelling mistakes")
        .mise_tool("pipx:codespell")
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
        .unsupported_configs(EDITORCONFIG_CHECKER_UNSUPPORTED_CONFIGS)
        .desc("Check files comply with EditorConfig settings")
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
    .desc("Lint Go code; uses --new-from-rev to scope analysis to changed code")
    .lang()
}

fn check_ruff() -> Check {
    Check::file("ruff", "ruff check {FILE}", &["*.py"])
        .fix("ruff check --fix {FILE}")
        .linter_config("ruff.toml", "--config")
        .baseline_config(RUFF_BASELINE_CONFIG)
        .unsupported_configs(RUFF_UNSUPPORTED_CONFIGS)
        .migrate_tool_keys_after(
            V2_BASELINE_SETUP_VERSION,
            &["pipx:ruff", "github:astral-sh/ruff"],
        )
        .desc("Lint Python code")
        .lang()
}

fn check_ruff_format() -> Check {
    Check::file("ruff-fmt", "ruff format --check {FILE}", &["*.py"])
        .bin("ruff")
        .fix("ruff format {FILE}")
        .linter_config("ruff.toml", "--config")
        .baseline_config(RUFF_BASELINE_CONFIG)
        .unsupported_configs(RUFF_UNSUPPORTED_CONFIGS)
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
    .migrate_tool_keys_after(V2_BASELINE_SETUP_VERSION, &["npm:@biomejs/biome"])
    .desc("Lint JS/TS/JSON files")
    .lang()
}

fn check_biome_format() -> Check {
    Check::file(
        "biome-fmt",
        "biome format {FILE}",
        &["*.json", "*.jsonc", "*.js", "*.ts", "*.jsx", "*.tsx"],
    )
    .bin("biome")
    .fix("biome format --write {FILE}")
    .baseline_config(BIOME_BASELINE_CONFIG)
    .unsupported_configs(BIOME_UNSUPPORTED_CONFIGS)
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
    .desc("Lint Rust code; runs on all .rs files, not just changed")
    .lang()
}

fn check_cargo_fmt() -> Check {
    Check::project("cargo-fmt", "cargo fmt -- {CONFIG_ARGS} --check", &["*.rs"])
        .fix("cargo fmt -- {CONFIG_ARGS}")
        .linter_config("rustfmt.toml", "--config-path")
        .baseline_config(RUSTFMT_BASELINE_CONFIG)
        .unsupported_configs(RUSTFMT_UNSUPPORTED_CONFIGS)
        .bin("rustfmt")
        .mise_tool("rust")
        .toolchain_components("rustfmt")
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
    .mise_tool("github:google/google-java-format")
    .formatter()
    .editorconfig_line_length_off(
        &["*.java"],
        "Java line length is handled by google-java-format",
        Some(EditorconfigDirectiveStyle::Slash),
    )
    .migrate_tool_keys_after(
        V1_BOOTSTRAP_SETUP_VERSION,
        &["ubi:google/google-java-format"],
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
    .migrate_tool_keys_after(V1_BOOTSTRAP_SETUP_VERSION, &["ubi:pinterest/ktlint"])
    .migrate_tool_keys_after(V2_BASELINE_SETUP_VERSION, &["github:pinterest/ktlint"])
    .desc("Lint and format Kotlin code")
    .lang()
}

fn check_dotnet_format() -> Check {
    Check::files(
        "dotnet-fmt",
        "dotnet format --verify-no-changes --include {RELFILES}",
        &["*.cs"],
    )
    .fix("dotnet format --include {RELFILES}")
    .full_cmd("dotnet format --verify-no-changes", "dotnet format")
    .bin("dotnet")
    .mise_tool("dotnet")
    .toolchain()
    .formatter()
    .desc("Format C# code")
    .lang()
}

fn check_lychee() -> Check {
    Check::special_with_bin("lychee", "lychee", SpecialKind::Links, false)
        .desc("Check for broken links")
        .docs(
            "Orchestrates [lychee](https://lychee.cli.rs/) for link checking. \
            Requires `lychee` in `[tools]`.\n\
            \n\
            Default behavior: checks all links in changed files. When\n\
            `check_all_local = true` in `flint.toml`, adds a second pass over local links\n\
            in all files — useful when broken internal links from unchanged files also\n\
            matter.\n\
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
    Check::special_with_bin("renovate-deps", "renovate", SpecialKind::RenovateDeps, true)
        .adaptive()
        .mise_tool("npm:renovate")
        .patterns(RENOVATE_CONFIG_PATTERNS)
        .desc("Verify Renovate dependency snapshot is up to date")
        .docs(
            "Verifies `.github/renovate-tracked-deps.json` is up to date by running\n\
            Renovate locally and comparing its output against the committed snapshot.\n\
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
        )
}

fn check_license_header() -> Check {
    Check::special("license-header", SpecialKind::LicenseHeader, false)
        .activate_unconditionally()
        .desc("Check source files have the required license header")
}

fn check_flint_setup() -> Check {
    Check::special("flint-setup", SpecialKind::FlintSetup, true)
        .activate_unconditionally()
        .patterns(&["mise.toml"])
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
            With `--fix`, rewrites Flint-managed config in place and advances\n\
            `settings.setup_migration_version` when a migration applies.",
        )
}

pub fn builtin() -> Vec<Check> {
    vec![
        check_flint_setup(),
        check_shellcheck(),
        check_shfmt(),
        check_rumdl(),
        check_yaml_lint(),
        check_taplo(),
        check_actionlint(),
        check_hadolint(),
        check_xmllint(),
        check_codespell(),
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
