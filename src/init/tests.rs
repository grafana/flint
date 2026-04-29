use super::*;
use config_files::generate_flint_toml;
use detection::entry_components_differ;
use generation::{
    apply_changes, get_existing_config_dir, has_slow_selected, normalize_tools_section,
};
use scaffold::{apply_env_and_tasks, generate_lint_workflow};

#[test]
fn detect_obsolete_keys_finds_known_stale_key() {
    use detection::detect_obsolete_keys;
    let mut keys = HashSet::new();
    keys.insert("github:mvdan/sh".to_string());
    keys.insert("shellcheck".to_string());
    let found = detect_obsolete_keys(&keys);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].0, "github:mvdan/sh");
    assert_eq!(found[0].1, "shfmt");
    assert_eq!(found[1].0, "shellcheck");
    assert_eq!(found[1].1, "github:koalaman/shellcheck");
}

#[test]
fn detect_obsolete_keys_ignores_current_keys() {
    use detection::detect_obsolete_keys;
    let mut keys = HashSet::new();
    keys.insert("rumdl".to_string());
    keys.insert("github:koalaman/shellcheck".to_string());
    let found = detect_obsolete_keys(&keys);
    assert!(found.is_empty());
}

#[test]
fn all_registry_checks_have_install_key_or_none() {
    for check in builtin() {
        if check.uses_binary() && !check.activate_unconditionally {
            let key = install_key(&check);
            assert!(
                key.is_some(),
                "check '{}' is missing an install key",
                check.name
            );
        }
    }
}

#[test]
fn entry_components_differ_string_value() {
    let content = "[tools]\nrust = \"1.80.0\"\n";
    assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
}

#[test]
fn entry_components_differ_inline_table_without_components() {
    let content = "[tools]\nrust = { version = \"1.80.0\" }\n";
    assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
}

#[test]
fn entry_components_differ_inline_table_wrong_components() {
    let content = "[tools]\nrust = { version = \"1.80.0\", components = \"clippy\" }\n";
    assert!(entry_components_differ(content, "rust", "clippy,rustfmt"));
}

#[test]
fn entry_components_differ_inline_table_correct_components() {
    let content = "[tools]\nrust = { version = \"1.80.0\", components = \"clippy,rustfmt\" }\n";
    assert!(!entry_components_differ(content, "rust", "clippy,rustfmt"));
}

#[test]
fn normalize_tools_section_sorts_and_inserts_linters_header() {
    let content = r#"[tools]
lychee = "0.22.0"
actionlint = "1.7.0"
rumdl = "0.1.0"
rust = { version = "1.95.0", components = "clippy,rustfmt" }
"#;
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), content).unwrap();
    let changed = normalize_tools_section(tmp.path()).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    let header_pos = result.find("# Linters").expect("header present");
    let biome_pos = result.find("biome =").unwrap_or(usize::MAX);
    let rust_pos = result.find("rust =").expect("rust present");
    let actionlint_pos = result.find("actionlint =").expect("actionlint present");
    let lychee_pos = result.find("lychee =").expect("lychee present");
    let rumdl_pos = result.find("rumdl =").expect("rumdl present");
    assert!(rust_pos < header_pos, "toolchains above header");
    assert!(actionlint_pos > header_pos, "linters below header");
    assert!(
        actionlint_pos < lychee_pos
            && lychee_pos < rumdl_pos
            && (biome_pos == usize::MAX || rumdl_pos < biome_pos),
        "linters sorted alphabetically"
    );

    let changed_again = normalize_tools_section(tmp.path()).unwrap();
    assert!(!changed_again);
    let result_again = std::fs::read_to_string(tmp.path()).unwrap();
    assert_eq!(result, result_again);
}

#[test]
fn normalize_tools_section_moves_node_above_linters_header() {
    let content = r#"[tools]
rust = { version = "1.95.0", components = "clippy,rustfmt" }

# Linters
bats = "1.13.0"
java = "temurin-25.0.2+10.0.LTS"
node = "24.15.0"
"npm:renovate" = "43.0.0"
"github:koalaman/shellcheck" = "0.11.0"
"#;
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), content).unwrap();
    let changed = normalize_tools_section(tmp.path()).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    let bats_pos = result.find("bats =").expect("bats present");
    let java_pos = result.find("java =").expect("java present");
    let node_pos = result.find("node =").expect("node present");
    let header_pos = result.find("# Linters").expect("header present");
    let renovate_pos = result.find("\"npm:renovate\"").expect("renovate present");
    assert!(
        bats_pos < header_pos
            && java_pos < header_pos
            && node_pos < header_pos
            && header_pos < renovate_pos,
        "non-linter tools must stay above linter header:\n{result}"
    );
    assert_eq!(result.matches("# Linters").count(), 1, "single header");
}

#[test]
fn normalize_tools_section_preserves_unrelated_tool_comments() {
    let content = r#"[tools]
# Runtime comment
node = "24.15.0"

# Linters
"github:koalaman/shellcheck" = "0.11.0"
"#;
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), content).unwrap();
    normalize_tools_section(tmp.path()).unwrap();
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(result.contains("# Runtime comment"));
    assert!(result.contains("# Linters"));
    assert_eq!(result.matches("# Linters").count(), 1);
}

#[test]
fn normalize_tools_section_keeps_unknown_tools_above_linters_header() {
    let content = r#"[tools]

# Linters
custom-tool = "1.0.0"
java = "temurin-25.0.3+9.0.LTS"
node = "24.15.0"
protoc = "34.1"
"github:koalaman/shellcheck" = "0.11.0"
"#;
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), content).unwrap();
    let changed = normalize_tools_section(tmp.path()).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    let custom_pos = result.find("custom-tool =").expect("custom tool present");
    let java_pos = result.find("java =").expect("java present");
    let node_pos = result.find("node =").expect("node present");
    let protoc_pos = result.find("protoc =").expect("protoc present");
    let header_pos = result.find("# Linters").expect("header present");
    let shellcheck_pos = result
        .find("\"github:koalaman/shellcheck\" =")
        .expect("shellcheck present");
    assert!(
        custom_pos < header_pos
            && java_pos < header_pos
            && node_pos < header_pos
            && protoc_pos < header_pos
            && header_pos < shellcheck_pos,
        "only explicitly managed linter keys belong below the header:\n{result}"
    );
    assert_eq!(result.matches("# Linters").count(), 1, "single header");
}

#[test]
fn normalize_tools_section_sorts_cargo_flint_pin_with_linters() {
    let content = r#"[tools]
rust = { version = "1.95.0", components = "clippy,rustfmt" }

# Linters
actionlint = "1.7.12"
biome = "2.4.12"
editorconfig-checker = "3.6.1"
"cargo:https://github.com/grafana/flint" = "rev:deadbeef"
taplo = "0.10.0"
"#;
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), content).unwrap();
    let changed = normalize_tools_section(tmp.path()).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    let biome_pos = result.find("biome =").expect("biome present");
    let flint_pos = result
        .find("\"cargo:https://github.com/grafana/flint\" =")
        .expect("flint present");
    let ec_pos = result
        .find("editorconfig-checker =")
        .expect("editorconfig-checker present");
    let taplo_pos = result.find("taplo =").expect("taplo present");
    assert!(
        biome_pos < flint_pos && flint_pos < ec_pos && ec_pos < taplo_pos,
        "cargo flint pin should sort with the linter block:\n{result}"
    );
}

#[test]
fn apply_changes_upgrade_preserves_version() {
    let content = "[tools]\nrust = \"1.80.0\"\n";
    let tmp = tempfile::NamedTempFile::new().unwrap();
    apply_changes(
        tmp.path(),
        content,
        &[],
        &[],
        &[("rust".to_string(), "clippy,rustfmt".to_string())],
    )
    .unwrap();
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(result.contains("version = \"1.80.0\""), "version preserved");
    assert!(
        result.contains("components = \"clippy,rustfmt\""),
        "components added"
    );
}

#[test]
fn parse_tool_keys_reads_simple_toml() {
    let content = r#"
[tools]
"github:koalaman/shellcheck" = "v0.11.0"
rumdl = "0.1.0"
rust = { version = "1.0", components = "clippy" }
"#;
    let keys = parse_tool_keys(content);
    assert!(keys.contains("github:koalaman/shellcheck"));
    assert!(keys.contains("rumdl"));
    assert!(keys.contains("rust"));
    assert!(!keys.contains("nonexistent"));
}

#[test]
fn compute_desired_tools_lang_profile() {
    let registry = builtin();
    let mut present = HashSet::new();
    present.insert("*.sh".to_string());
    present.insert("*.bash".to_string());
    present.insert("*.rs".to_string());
    let categories = profile_to_categories(Profile::Lang);
    let tools = compute_desired_tools(&registry, &present, &categories);
    assert!(!tools.contains_key("shellcheck"));
    assert!(!tools.contains_key("shfmt"));
    assert!(tools.contains_key("rust"));
    assert!(!tools.contains_key("pipx:codespell"));
}

#[test]
fn rust_install_entry_has_components() {
    let registry = builtin();
    let mut present = HashSet::new();
    present.insert("*.rs".to_string());
    let categories = profile_to_categories(Profile::Lang);
    let tools = compute_desired_tools(&registry, &present, &categories);
    assert_eq!(
        tools.get("rust"),
        Some(&Some("clippy,rustfmt".to_string())),
        "rust tool entry should carry merged components"
    );
}

#[test]
fn compute_desired_tools_default_excludes_slow() {
    let registry = builtin();
    let present: HashSet<String> = HashSet::new();
    let categories = profile_to_categories(Profile::Default);
    let tools = compute_desired_tools(&registry, &present, &categories);
    assert!(!tools.contains_key("npm:renovate"));
    assert!(tools.contains_key("lychee"));
}

#[test]
fn compute_desired_tools_comprehensive_includes_slow() {
    let registry = builtin();
    let mut present: HashSet<String> = HashSet::new();
    present.insert(".github/renovate.json5".to_string());
    let categories = profile_to_categories(Profile::Comprehensive);
    let tools = compute_desired_tools(&registry, &present, &categories);
    assert!(tools.contains_key("lychee"));
    assert!(tools.contains_key("npm:renovate"));
}

#[test]
fn renovate_deps_absent_without_renovate_config() {
    let registry = builtin();
    let present: HashSet<String> = HashSet::new();
    let categories = profile_to_categories(Profile::Comprehensive);
    let tools = compute_desired_tools(&registry, &present, &categories);
    assert!(!tools.contains_key("npm:renovate"));
}

#[test]
fn has_slow_selected_false_for_default_profile() {
    let registry = builtin();
    let present = HashSet::new();
    let categories = profile_to_categories(Profile::Default);
    let groups = build_linter_groups(&registry, &present, &HashSet::new(), "", &categories);
    assert!(!has_slow_selected(&groups));
}

#[test]
fn get_existing_config_dir_reads_env_section() {
    let content = "[env]\nFLINT_CONFIG_DIR = \".github/config\"\n";
    assert_eq!(
        get_existing_config_dir(content),
        Some(".github/config".to_string())
    );
}

#[test]
fn get_existing_config_dir_absent() {
    let content = "[tools]\nrust = \"latest\"\n";
    assert_eq!(get_existing_config_dir(content), None);
}

#[test]
fn generate_rumdl_config_writes_file() {
    use hooks::rumdl::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    let written = generate_config(tmp.path(), &config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(config_dir.join(".rumdl.toml")).unwrap();
    assert!(content.contains("line-length = 120"));
    assert!(content.contains("code-blocks = false"));
    assert!(content.contains("[MD060]"));
    assert!(content.contains("style = \"aligned\""));
    assert!(!content.contains("[global]"));
}

#[test]
fn generate_rumdl_config_skips_when_target_exists() {
    use hooks::rumdl::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join(".rumdl.toml"), "existing").unwrap();
    let written = generate_config(tmp.path(), &config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(!written);
    let content = std::fs::read_to_string(config_dir.join(".rumdl.toml")).unwrap();
    assert_eq!(content, "existing");
}

#[test]
fn generate_rumdl_config_replaces_legacy_json() {
    use hooks::rumdl::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    std::fs::write(tmp.path().join(".markdownlint.json"), r#"{"MD013":false}"#).unwrap();
    let written = generate_config(tmp.path(), &config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(written);
    assert!(!tmp.path().join(".markdownlint.json").exists());
    let content = std::fs::read_to_string(config_dir.join(".rumdl.toml")).unwrap();
    assert!(content.contains("[global]"));
    assert!(content.contains("disable = [\"line-length\"]"));
}

#[test]
fn generate_rumdl_config_converts_legacy_yaml() {
    use hooks::rumdl::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    std::fs::write(
        tmp.path().join(".markdownlint.yaml"),
        r#"
ul-style: false
line-length: false
no-duplicate-heading:
  siblings_only: true
ol-prefix:
  style: ordered
no-inline-html: false
fenced-code-language: false
no-trailing-punctuation:
  punctuation: ".,;:"
MD059: false
MD041: false
"#,
    )
    .unwrap();
    let written = generate_config(tmp.path(), &config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(written);
    assert!(!tmp.path().join(".markdownlint.yaml").exists());
    let content = std::fs::read_to_string(config_dir.join(".rumdl.toml")).unwrap();
    assert!(content.contains("[global]"));
    assert!(content.contains("\"MD059\""));
    assert!(content.contains("\"line-length\""));
    assert!(content.contains("\"no-inline-html\""));
    assert!(content.contains("\"MD041\""));
    assert!(content.contains("\"ul-style\""));
    assert!(content.contains("\"fenced-code-language\""));
    assert!(content.contains("[no-duplicate-heading]"));
    assert!(content.contains("siblings-only = true"));
    assert!(content.contains("[no-trailing-punctuation]"));
    assert!(content.contains("punctuation = \".,;:\""));
    assert!(content.contains("[ol-prefix]"));
    assert!(content.contains("style = \"ordered\""));
}

#[test]
fn remove_legacy_lint_files_removes_v1_artifacts() {
    use config_files::remove_legacy_lint_files;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(tmp.path().join(".prettierignore"), "docs/themes/**\n").unwrap();
    std::fs::write(tmp.path().join(".gitleaksignore"), "secret\n").unwrap();
    std::fs::write(config_dir.join("super-linter.env"), "LOG_LEVEL=ERROR\n").unwrap();

    let removed = remove_legacy_lint_files(tmp.path(), &config_dir).unwrap();
    assert_eq!(removed.len(), 3);
    assert!(!tmp.path().join(".prettierignore").exists());
    assert!(!tmp.path().join(".gitleaksignore").exists());
    assert!(!config_dir.join("super-linter.env").exists());
}

#[test]
fn remove_stale_markdownlint_line_length_directives_strips_md013_only() {
    use config_files::remove_stale_markdownlint_line_length_directives;
    let tmp = tempfile::TempDir::new().unwrap();
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::fs::write(
        tmp.path().join("README.md"),
        "# Title\n\n<!-- markdownlint-disable MD013 -->\nlong line\n<!-- markdownlint-enable MD013 -->\n<!-- markdownlint-disable MD033 -->\nhtml\n<!-- markdownlint-enable MD033 -->\n",
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let changed = remove_stale_markdownlint_line_length_directives(tmp.path()).unwrap();
    assert_eq!(changed, vec!["README.md".to_string()]);
    let updated = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();
    assert!(!updated.contains("markdownlint-disable MD013"));
    assert!(!updated.contains("markdownlint-enable MD013"));
    assert!(updated.contains("markdownlint-disable MD033"));
    assert!(updated.contains("markdownlint-enable MD033"));
}

#[test]
fn remove_stale_editorconfig_checker_directives_strips_delegated_markdown_comments() {
    use crate::registry::EditorconfigDirectiveStyle;
    use config_files::remove_stale_editorconfig_checker_directives;
    let tmp = tempfile::TempDir::new().unwrap();
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::fs::write(
        tmp.path().join("README.md"),
        "# Title\n\n<!-- editorconfig-checker-disable -->\n- [Link](https://example.com) <!-- editorconfig-checker-disable-line -->\n<!-- editorconfig-checker-enable -->\n",
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let changed = remove_stale_editorconfig_checker_directives(
        tmp.path(),
        &[(&["*.md"], EditorconfigDirectiveStyle::Html)],
    )
    .unwrap();
    assert_eq!(changed, vec!["README.md".to_string()]);
    let updated = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();
    assert!(!updated.contains("editorconfig-checker-disable"));
    assert!(!updated.contains("editorconfig-checker-enable"));
    assert!(updated.contains("- [Link](https://example.com)"));
}

#[test]
fn generate_editorconfig_writes_file() {
    use config_files::generate_editorconfig;
    let tmp = tempfile::TempDir::new().unwrap();
    let written = generate_editorconfig(tmp.path(), DEFAULT_LINE_LENGTH).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
    assert!(content.contains("max_line_length = 120"));
    assert!(content.contains("insert_final_newline = true"));
}

#[test]
fn generate_editorconfig_patches_existing_global_section() {
    use config_files::generate_editorconfig;
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join(".editorconfig"),
        "root = true\n\n[*]\nindent_size = 2\n\n[*.rs]\nindent_size = 4\n",
    )
    .unwrap();
    let written = generate_editorconfig(tmp.path(), DEFAULT_LINE_LENGTH).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
    assert!(content.contains("[*]\nindent_size = 2\nmax_line_length = 120\n"));
    assert!(content.contains("[*.rs]\nindent_size = 4\n"));
}

#[test]
fn generate_editorconfig_skips_existing_line_length() {
    use config_files::generate_editorconfig;
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join(".editorconfig"),
        "root = true\n\n[*]\nmax_line_length = 100\n",
    )
    .unwrap();
    let written = generate_editorconfig(tmp.path(), DEFAULT_LINE_LENGTH).unwrap();
    assert!(!written);
    let content = std::fs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
    assert!(content.contains("max_line_length = 100"));
    assert!(!content.contains("max_line_length = 120"));
}

#[test]
fn disable_editorconfig_line_length_for_patterns_updates_editorconfig() {
    use config_files::disable_editorconfig_line_length_for_patterns;
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join(".editorconfig"),
        "root = true\n\n[*]\nmax_line_length = 120\n",
    )
    .unwrap();
    let changed = disable_editorconfig_line_length_for_patterns(
        tmp.path(),
        &[(&["*.md"], "Markdown line length is handled by rumdl")],
    )
    .unwrap();
    assert_eq!(changed, vec!["[*.md]".to_string()]);
    let content = std::fs::read_to_string(tmp.path().join(".editorconfig")).unwrap();
    assert!(content.contains("[*.md]"));
    assert!(content.contains("# Markdown line length is handled by rumdl"));
    assert!(content.contains("max_line_length = off"));
}

#[test]
fn disable_editorconfig_line_length_for_patterns_is_idempotent() {
    use config_files::disable_editorconfig_line_length_for_patterns;
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join(".editorconfig"),
        "root = true\n\n[*]\nmax_line_length = 120\n\n[*.md]\n# Markdown line length is handled by rumdl\nmax_line_length = off\n",
    )
    .unwrap();
    let changed = disable_editorconfig_line_length_for_patterns(
        tmp.path(),
        &[(&["*.md"], "Markdown line length is handled by rumdl")],
    )
    .unwrap();
    assert!(changed.is_empty());
}

#[test]
fn generate_yamllint_config_writes_file() {
    use hooks::yamllint::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    let written = generate_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(config_dir.join(".yamllint.yml")).unwrap();
    assert_eq!(
        content,
        "extends: relaxed\n\nrules:\n  document-start: disable\n  line-length:\n    max: 120\n  indentation: enable\n"
    );
}

#[test]
fn generate_taplo_config_writes_file() {
    use hooks::taplo::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    let written = generate_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(config_dir.join(".taplo.toml")).unwrap();
    assert!(content.contains("[formatting]"));
    assert!(content.contains("column_width = 120"));
    assert!(content.contains("indent_string = \"  \""));
}

#[test]
fn generate_taplo_config_skips_existing_supported_file() {
    use hooks::taplo::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join(".taplo.toml"), "existing").unwrap();
    let written = generate_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(!written);
    let content = std::fs::read_to_string(config_dir.join(".taplo.toml")).unwrap();
    assert_eq!(content, "existing");
}

#[test]
fn generate_taplo_config_skips_existing_legacy_name() {
    use hooks::taplo::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("taplo.toml"), "existing").unwrap();
    let written = generate_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(!written);
    assert!(!config_dir.join(".taplo.toml").exists());
}

#[test]
fn generate_rustfmt_config_writes_file() {
    use hooks::rustfmt::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    let written = generate_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(config_dir.join("rustfmt.toml")).unwrap();
    assert_eq!(content, "max_width = 120\n");
}

#[test]
fn generate_rustfmt_config_skips_existing_file() {
    use hooks::rustfmt::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join(".github/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("rustfmt.toml"), "existing").unwrap();
    let written = generate_config(&config_dir, DEFAULT_LINE_LENGTH).unwrap();
    assert!(!written);
    let content = std::fs::read_to_string(config_dir.join("rustfmt.toml")).unwrap();
    assert_eq!(content, "existing");
}

#[test]
fn generate_biome_config_writes_file() {
    use crate::linters::biome::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    let written = generate_config(tmp.path()).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(tmp.path().join("biome.jsonc")).unwrap();
    assert!(content.contains("\"indentStyle\": \"space\""));
    assert!(content.contains("\"indentWidth\": 2"));
}

#[test]
fn generate_biome_config_skips_existing_jsonc() {
    use crate::linters::biome::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("biome.jsonc"), "existing").unwrap();
    let written = generate_config(tmp.path()).unwrap();
    assert!(!written);
    let content = std::fs::read_to_string(tmp.path().join("biome.jsonc")).unwrap();
    assert_eq!(content, "existing");
}

#[test]
fn generate_biome_config_migrates_legacy_supported_json_name() {
    use crate::linters::biome::generate_config;
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("biome.json"), "existing").unwrap();
    let written = generate_config(tmp.path()).unwrap();
    assert!(written);
    assert!(!tmp.path().join("biome.json").exists());
    let content = std::fs::read_to_string(tmp.path().join("biome.jsonc")).unwrap();
    assert_eq!(content, "existing");
}

#[test]
fn generate_flint_toml_writes_skeleton() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().join("config");
    let written =
        generate_flint_toml(&dir, "main", crate::setup::V2_BASELINE_SETUP_VERSION).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(dir.join("flint.toml")).unwrap();
    assert!(content.contains("[settings]"));
    assert!(content.contains("# exclude ="));
    assert!(!content.contains("base_branch"));
}

#[test]
fn generate_flint_toml_non_main_branch() {
    let tmp = tempfile::TempDir::new().unwrap();
    let written = generate_flint_toml(
        tmp.path(),
        "master",
        crate::setup::V2_BASELINE_SETUP_VERSION,
    )
    .unwrap();
    assert!(written);
    let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
    assert!(content.contains("base_branch = \"master\""));
}

#[test]
fn generate_flint_toml_skips_existing() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("flint.toml"), "existing content").unwrap();
    let written =
        generate_flint_toml(tmp.path(), "main", crate::setup::V2_BASELINE_SETUP_VERSION).unwrap();
    assert!(!written);
    let content = std::fs::read_to_string(tmp.path().join("flint.toml")).unwrap();
    assert_eq!(content, "existing content");
}

#[test]
fn generate_lint_workflow_writes_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let written = generate_lint_workflow(tmp.path(), "main", false).unwrap();
    assert!(written);
    let content = std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
    assert!(content.contains("branches: [main]"));
    assert!(content.contains("mise run lint"));
    assert!(content.contains("fetch-depth: 0"));
    assert!(content.contains("persist-credentials: false"));
    assert!(content.contains("mise-action"));
    assert!(content.contains("GITHUB_REPOSITORY: ${{ github.repository }}"));
    assert!(content.contains("GITHUB_BASE_REF: ${{ github.base_ref }}"));
    assert!(content.contains("GITHUB_HEAD_REF: ${{ github.head_ref }}"));
    assert!(content.contains(
        "PR_HEAD_REPO: ${{ github.event.pull_request.head.repo.full_name || github.repository }}"
    ));
    assert!(!content.contains("GITHUB_HEAD_SHA"));
    assert!(content.contains("github.token"));
    assert!(content.contains("pull_request.head.repo.full_name"));
    assert!(!content.contains("rust-cache"));
    assert!(!content.contains("rustup component"));
}

#[test]
fn generate_lint_workflow_non_main_branch() {
    let tmp = tempfile::TempDir::new().unwrap();
    generate_lint_workflow(tmp.path(), "master", false).unwrap();
    let content = std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
    assert!(content.contains("branches: [master]"));
}

#[test]
fn generate_lint_workflow_skips_existing() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
    std::fs::write(
        tmp.path().join(".github/workflows/lint.yml"),
        "existing content",
    )
    .unwrap();
    let written = generate_lint_workflow(tmp.path(), "main", false).unwrap();
    assert!(!written);
    let content = std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
    assert_eq!(content, "existing content");
}

#[test]
fn generate_lint_workflow_with_rust() {
    let tmp = tempfile::TempDir::new().unwrap();
    generate_lint_workflow(tmp.path(), "main", true).unwrap();
    let content = std::fs::read_to_string(tmp.path().join(".github/workflows/lint.yml")).unwrap();
    assert!(content.contains("Swatinem/rust-cache"));
    assert!(content.contains("rustup component add clippy rustfmt"));
    assert!(content.contains("warms the Rust cache"));
}

#[test]
fn apply_env_and_tasks_adds_sections() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "[tools]\nrust = \"latest\"\n").unwrap();
    let changed = apply_env_and_tasks(tmp.path(), ".github/config", false, &[]).unwrap();
    assert!(changed);
    let content = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(content.contains("FLINT_CONFIG_DIR = \".github/config\""));
    assert!(content.contains("flint run"));
    assert!(content.contains("flint run --fix"));
    assert!(!content.contains("--fast-only"));
}

#[test]
fn apply_env_and_tasks_does_not_add_pre_commit_task_when_slow() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "").unwrap();
    apply_env_and_tasks(tmp.path(), ".", true, &[]).unwrap();
    let content = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(!content.contains("--fast-only"));
    assert!(!content.contains("lint:pre-commit"));
}

#[test]
fn apply_env_and_tasks_idempotent() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "").unwrap();
    apply_env_and_tasks(tmp.path(), ".github/config", false, &[]).unwrap();
    let after_first = std::fs::read_to_string(tmp.path()).unwrap();
    let changed = apply_env_and_tasks(tmp.path(), ".github/config", false, &[]).unwrap();
    assert!(!changed);
    let after_second = std::fs::read_to_string(tmp.path()).unwrap();
    assert_eq!(after_first, after_second);
}

#[test]
fn apply_env_and_tasks_replaces_stale_lint_task() {
    let content = r#"
[tasks."lint"]
description = "Run all lints"
depends = ["lint:fast", "lint:renovate-deps"]
"#;
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), content).unwrap();
    let removed = vec!["lint:renovate-deps".to_string()];
    apply_env_and_tasks(tmp.path(), ".github/config", false, &removed).unwrap();
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(
        result.contains("run = \"flint run\""),
        "stale lint task replaced: {result}"
    );
    assert!(
        !result.contains("depends"),
        "old depends array removed: {result}"
    );
}
