use super::*;

#[test]
fn configure_renovate_deps_appends_placeholder() {
    let tmp = write_tmp("[settings]\n");
    let changed = configure_renovate_deps_config(tmp.path()).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(result.contains("[checks.renovate-deps]"));
    assert!(result.contains("# exclude_managers = []"));
}

#[test]
fn configure_renovate_deps_keeps_existing_config() {
    let tmp = write_tmp("[checks.renovate-deps]\nexclude_managers = [\"npm\"]\n");
    let changed = configure_renovate_deps_config(tmp.path()).unwrap();
    assert!(!changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(result.contains("exclude_managers = [\"npm\"]"));
}

#[test]
fn configure_extract_workaround_env_adds_node_import() {
    let mut env = vec![];

    configure_extract_workaround_env(&mut env, "extract").unwrap();

    let node_options = env
        .iter()
        .find(|(key, _)| key == "NODE_OPTIONS")
        .map(|(_, value)| value)
        .unwrap();
    assert!(node_options.contains("--import="));
    assert!(node_options.contains("file://"));
}

#[test]
fn configure_extract_workaround_env_preserves_existing_node_options() {
    let mut env = vec![("NODE_OPTIONS".to_string(), "--trace-warnings".to_string())];

    configure_extract_workaround_env(&mut env, "extract").unwrap();

    let node_options = env
        .iter()
        .find(|(key, _)| key == "NODE_OPTIONS")
        .map(|(_, value)| value)
        .unwrap();
    assert!(node_options.contains("--trace-warnings"));
    assert!(node_options.contains("--import="));
    assert!(node_options.contains("file://"));
}

#[test]
fn replaces_unpinned_flint_entry_in_place() {
    let input = r#"{ extends: ["config:recommended", "github>grafana/flint"] }"#;
    let tmp = write_tmp(input);
    let changed = patch_renovate_extends(tmp.path()).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(
        result.contains("github>grafana/flint#v"),
        "pinned entry written: {result}"
    );
    assert_eq!(
        result.matches("grafana/flint").count(),
        1,
        "no duplicate: {result}"
    );
    assert!(
        !result.contains("\"github>grafana/flint\""),
        "unpinned removed: {result}"
    );
}

#[test]
fn replaces_differently_pinned_flint_entry() {
    let input = r#"{ extends: ["config:recommended", "github>grafana/flint#v0.5.0"] }"#;
    let tmp = write_tmp(input);
    let changed = patch_renovate_extends(tmp.path()).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(!result.contains("v0.5.0"), "old pin removed: {result}");
    assert_eq!(
        result.matches("grafana/flint").count(),
        1,
        "no duplicate: {result}"
    );
}

#[test]
fn no_op_when_already_pinned_to_current_version() {
    let entry = flint_preset();
    let input = format!(r#"{{ extends: ["config:recommended", "{entry}"] }}"#);
    let tmp = write_tmp(&input);
    let changed = patch_renovate_extends(tmp.path()).unwrap();
    assert!(!changed);
}

#[test]
fn adds_to_single_line_extends() {
    let input = r#"{ "extends": ["config:recommended"], "other": 1 }"#;
    let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
    assert!(result.contains(r#"["github>grafana/flint#v0.9.2", "config:recommended"]"#));
}

#[test]
fn adds_to_json5_unquoted_key() {
    let input = "{\n  extends: [\"config:recommended\"],\n}\n";
    let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
    assert!(result.contains(r#""github>grafana/flint#v0.9.2", "config:recommended""#));
}

#[test]
fn adds_to_multiline_extends() {
    let input = "{\n  extends: [\n    \"config:recommended\",\n    \"other\"\n  ]\n}\n";
    let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
    assert!(result.contains("\"github>grafana/flint#v0.9.2\","));
    let flint_pos = result.find("grafana/flint").unwrap();
    let existing_pos = result.find("config:recommended").unwrap();
    assert!(flint_pos < existing_pos);
}

#[test]
fn adds_extends_when_absent() {
    let input = "{\n  \"branchPrefix\": \"renovate/\"\n}\n";
    let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
    assert!(result.contains("\"extends\""));
    assert!(result.contains("github>grafana/flint#v0.9.2"));
}

#[test]
fn adds_extends_when_absent_in_empty_object() {
    let input = "{}\n";
    let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
    assert_eq!(
        result,
        "{\n  \"extends\": [\"github>grafana/flint#v0.9.2\"]}\n"
    );
}

#[test]
fn adds_to_empty_extends_array() {
    let input = r#"{ "extends": [] }"#;
    let result = add_to_extends(input, "github>grafana/flint#v0.9.2").unwrap();
    assert!(result.contains(r#"["github>grafana/flint#v0.9.2"]"#));
}

#[test]
fn ci_requires_github_token_or_github_com_token() {
    let err = validate_env(&[("CI", "true")]).unwrap_err();

    assert!(err.contains("GITHUB_COM_TOKEN"), "unexpected error:\n{err}");
    assert!(err.contains("GITHUB_TOKEN"), "unexpected error:\n{err}");
}

#[test]
fn ci_accepts_github_token() {
    let result = validate_env(&[("CI", "true"), ("GITHUB_TOKEN", "token")]);

    assert!(result.is_ok(), "unexpected validation error: {result:?}");
}

#[test]
fn ci_accepts_github_com_token() {
    let result = validate_env(&[("CI", "true"), ("GITHUB_COM_TOKEN", "token")]);

    assert!(result.is_ok(), "unexpected validation error: {result:?}");
}

#[test]
fn non_ci_missing_github_token_warns_without_failing() {
    let warning = validate_env(&[]).unwrap().unwrap();

    assert!(warning.contains("renovate-deps"));
    assert!(warning.contains("GITHUB_TOKEN"));
}
