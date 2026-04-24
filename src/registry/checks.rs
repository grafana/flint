use super::types::{Check, ConfigFile, SpecialKind};
use crate::linters::renovate_deps::RENOVATE_CONFIG_PATTERNS;

const TOOL_RUMDL: &[&str] = &["tool", "rumdl"];
const TOOL_CODESPELL: &[&str] = &["tool", "codespell"];
const TOOL_RUFF: &[&str] = &["tool", "ruff"];

const SHELLCHECK_BASELINE_CONFIGS: &[ConfigFile] = &[ConfigFile::config_dir(".shellcheckrc")];
const SHELLCHECK_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("shellcheckrc"),
    ConfigFile::project("shellcheckrc"),
];
const RUMDL_BASELINE_CONFIGS: &[ConfigFile] = &[ConfigFile::config_dir(".rumdl.toml")];
const RUMDL_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("rumdl.toml"),
    ConfigFile::project("rumdl.toml"),
    ConfigFile::project(".config/rumdl.toml"),
    ConfigFile::project_toml_section("pyproject.toml", TOOL_RUMDL),
];
const YAMLLINT_BASELINE_CONFIGS: &[ConfigFile] = &[ConfigFile::config_dir(".yamllint.yml")];
const YAMLLINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".yamllint"),
    ConfigFile::config_dir(".yamllint.yaml"),
    ConfigFile::project(".yamllint"),
    ConfigFile::project(".yamllint.yaml"),
];
const ACTIONLINT_BASELINE_CONFIGS: &[ConfigFile] = &[ConfigFile::config_dir("actionlint.yml")];
const ACTIONLINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir("actionlint.yaml"),
    ConfigFile::project(".github/actionlint.yaml"),
    ConfigFile::project(".github/actionlint.yml"),
];
const HADOLINT_BASELINE_CONFIGS: &[ConfigFile] = &[ConfigFile::config_dir(".hadolint.yaml")];
const HADOLINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".hadolint.yml"),
    ConfigFile::project(".hadolint.yml"),
];
const CODESPELL_BASELINE_CONFIGS: &[ConfigFile] = &[ConfigFile::config_dir(".codespellrc")];
const CODESPELL_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::project_ini_section("setup.cfg", "codespell"),
    ConfigFile::project_toml_section("pyproject.toml", TOOL_CODESPELL),
];
const EDITORCONFIG_CHECKER_BASELINE_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".editorconfig-checker.json"),
    ConfigFile::project(".editorconfig"),
];
const EDITORCONFIG_CHECKER_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".ecrc"),
    ConfigFile::project(".ecrc"),
];
const GOLANGCI_LINT_BASELINE_CONFIGS: &[ConfigFile] = &[ConfigFile::config_dir(".golangci.yml")];
const GOLANGCI_LINT_UNSUPPORTED_CONFIGS: &[ConfigFile] = &[
    ConfigFile::config_dir(".golangci.yaml"),
    ConfigFile::config_dir(".golangci.toml"),
    ConfigFile::config_dir(".golangci.json"),
    ConfigFile::project(".golangci.yaml"),
    ConfigFile::project(".golangci.toml"),
    ConfigFile::project(".golangci.json"),
];
const RUFF_BASELINE_CONFIGS: &[ConfigFile] = &[ConfigFile::config_dir("ruff.toml")];
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
/// - `cargo:yaml-lint` → name `yaml-lint`
/// - `github:pinterest/ktlint` → name `ktlint`
///
/// Exception: when the mise tool key is a language toolchain shared across multiple
/// binaries (e.g. `rust`, `go`, `dotnet`), use the binary name instead — the toolchain
/// name would be ambiguous (`rust` can't name both `cargo-fmt` and `cargo-clippy`).
fn check_shellcheck() -> Check {
    Check::file(
        "shellcheck",
        "shellcheck -x -P SCRIPTDIR {FILE}",
        &["*.sh", "*.bash", "*.bats"],
    )
    .linter_config(".shellcheckrc", "--rcfile")
    .baseline_configs(SHELLCHECK_BASELINE_CONFIGS)
    .unsupported_configs(SHELLCHECK_UNSUPPORTED_CONFIGS)
    .desc("Lint shell scripts for common mistakes")
    .style()
}

fn check_shfmt() -> Check {
    Check::file("shfmt", "shfmt -d {FILE}", &["*.sh", "*.bash"])
        .fix("shfmt -w {FILE}")
        .formatter()
        .desc("Format shell scripts")
        .style()
}

fn check_rumdl() -> Check {
    Check::file("rumdl", "rumdl check {FILE}", &["*.md"])
        .fix("rumdl check --fix {FILE}")
        .linter_config(".rumdl.toml", "--config")
        .baseline_configs(RUMDL_BASELINE_CONFIGS)
        .unsupported_configs(RUMDL_UNSUPPORTED_CONFIGS)
        .formatter()
        .desc("Lint Markdown files for style and consistency")
        .mise_tool("rumdl")
}

fn check_yaml_lint() -> Check {
    Check::files("yaml-lint", "yaml-lint {FILES}", &["*.yml", "*.yaml"])
        .fix("yaml-lint --fix {FILES}")
        .linter_config(".yamllint.yml", "-c")
        .baseline_configs(YAMLLINT_BASELINE_CONFIGS)
        .unsupported_configs(YAMLLINT_UNSUPPORTED_CONFIGS)
        .formatter()
        .desc("Lint YAML files for style and consistency")
        .mise_tool("cargo:yaml-lint")
}

fn check_taplo() -> Check {
    Check::file("taplo", "taplo fmt --check {FILE}", &["*.toml"])
        .fix("taplo fmt {FILE}")
        .mise_tool("github:tamasfe/taplo")
        .stderr_filter_prefixes(&[" INFO taplo:"])
        .formatter()
        .desc("Format TOML files")
        .docs(
            "Formats TOML files with [Taplo](https://taplo.tamasfe.dev/).\n\
            \n\
            This check intentionally stays basic: it uses `taplo fmt --check` for \
            verification and `taplo fmt` for `--fix`. That keeps behavior aligned \
            with flint's existing formatter-style checks.\n\
            \n\
            Current caveat: Taplo's published docs currently advertise TOML 1.0.0 \
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
    .baseline_configs(ACTIONLINT_BASELINE_CONFIGS)
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
    .baseline_configs(HADOLINT_BASELINE_CONFIGS)
    .unsupported_configs(HADOLINT_UNSUPPORTED_CONFIGS)
    .desc("Lint Dockerfiles")
    .style()
}

fn check_xmllint() -> Check {
    Check::files("xmllint", "xmllint --noout {FILES}", &["*.xml"])
        .mise_tool("cargo:xmloxide")
        .desc("Validate XML files are well-formed")
}

fn check_codespell() -> Check {
    Check::files("codespell", "codespell {FILES}", &["*"])
        .fix("codespell --write-changes {FILES}")
        .linter_config(".codespellrc", "--config")
        .baseline_configs(CODESPELL_BASELINE_CONFIGS)
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
        .baseline_configs(EDITORCONFIG_CHECKER_BASELINE_CONFIGS)
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
    .baseline_configs(GOLANGCI_LINT_BASELINE_CONFIGS)
    .unsupported_configs(GOLANGCI_LINT_UNSUPPORTED_CONFIGS)
    .desc("Lint Go code; uses --new-from-rev to scope analysis to changed code")
    .lang()
}

fn check_ruff() -> Check {
    Check::file("ruff", "ruff check {FILE}", &["*.py"])
        .fix("ruff check --fix {FILE}")
        .linter_config("ruff.toml", "--config")
        .baseline_configs(RUFF_BASELINE_CONFIGS)
        .unsupported_configs(RUFF_UNSUPPORTED_CONFIGS)
        .desc("Lint Python code")
        .mise_tool("github:astral-sh/ruff")
        .lang()
}

fn check_ruff_format() -> Check {
    Check::file("ruff-format", "ruff format --check {FILE}", &["*.py"])
        .bin("ruff")
        .fix("ruff format {FILE}")
        .linter_config("ruff.toml", "--config")
        .baseline_configs(RUFF_BASELINE_CONFIGS)
        .unsupported_configs(RUFF_UNSUPPORTED_CONFIGS)
        .formatter()
        .desc("Format Python code")
        .mise_tool("github:astral-sh/ruff")
        .lang()
}

fn check_biome() -> Check {
    Check::file(
        "biome",
        "biome check {FILE}",
        &["*.json", "*.jsonc", "*.js", "*.ts", "*.jsx", "*.tsx"],
    )
    .fix("biome check --fix {FILE}")
    .desc("Lint JS/TS/JSON files")
    .mise_tool("biome")
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
    Check::project("cargo-fmt", "cargo fmt -- --check", &["*.rs"])
        .fix("cargo fmt")
        .bin("rustfmt")
        .mise_tool("rust")
        .toolchain_components("rustfmt")
        .formatter()
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
    .mise_tool("github:pinterest/ktlint")
    .windows_java_jar()
    .formatter()
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
    .formatter()
    .desc("Format C# code")
    .lang()
}

fn check_lychee() -> Check {
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
        )
}

fn check_renovate_deps() -> Check {
    Check::special("renovate-deps", "renovate", SpecialKind::RenovateDeps)
        .adaptive()
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
        )
}

fn check_license_header() -> Check {
    Check::special(
        "license-header",
        "license-header",
        SpecialKind::LicenseHeader,
    )
    .activate_unconditionally()
    .desc("Check source files have the required license header")
}

pub fn builtin() -> Vec<Check> {
    vec![
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
