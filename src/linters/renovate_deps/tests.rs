use super::install_patch::configure_extract_workaround_env;
use super::mise_normalize::patch_semver_equivalent_mise_values;
use super::rules::{
    ComparablePackageRule, ExtractVersionMismatch, RuleMatcher, incomplete_meta_for_rules,
    relevant_dep_names, validate_extract_version_consistency,
};
use super::snapshot::{DepFiles, DepMeta};
use super::*;
use std::collections::BTreeSet;

type FileManagers<'a> = [(&'a str, &'a [(&'a str, &'a [&'a str])])];

fn log(config_json: &str) -> Vec<u8> {
    format!(r#"{{"msg":"Extracted dependencies","packageFiles":{config_json}}}"#).into_bytes()
}

fn log_current(config_json: &str) -> Vec<u8> {
    format!(r#"{{"msg":"packageFiles with updates","config":{config_json}}}"#).into_bytes()
}

fn dep_files(entries: &FileManagers<'_>) -> DepFiles {
    entries
        .iter()
        .map(|(file, managers)| {
            let m = managers
                .iter()
                .map(|(mgr, deps)| {
                    (
                        mgr.to_string(),
                        deps.iter().map(|d| d.to_string()).collect(),
                    )
                })
                .collect();
            (file.to_string(), m)
        })
        .collect()
}

fn snapshot(meta: &[(&str, Option<&str>, Option<&str>)], files: &FileManagers<'_>) -> Snapshot {
    Snapshot {
        meta: meta
            .iter()
            .map(|(dep, package_name, datasource)| {
                (
                    dep.to_string(),
                    DepMeta {
                        package_name: package_name.map(ToOwned::to_owned),
                        datasource: datasource.map(ToOwned::to_owned),
                        current_value: None,
                        current_version: None,
                        extract_version: None,
                    },
                )
            })
            .collect(),
        files: dep_files(files),
    }
}

fn validate_env(vars: &[(&str, &str)]) -> Result<Option<String>, String> {
    let vars: std::collections::HashMap<String, String> = vars
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    validate_runtime_env_from(|name| vars.get(name).cloned())
}

fn write_tmp(content: &str) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), content).unwrap();
    file
}

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

#[test]
fn extracts_deps_basic() {
    let log = log(
        r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"express"},{"depName":"lodash"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(
        result,
        snapshot(
            &[("express", None, None), ("lodash", None, None),],
            &[("package.json", &[("npm", &["express", "lodash"])])],
        )
    );
}

#[test]
fn extracts_deps_from_current_renovate_message() {
    let log = log_current(
        r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"express"},{"depName":"lodash"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(
        result,
        snapshot(
            &[("express", None, None), ("lodash", None, None),],
            &[("package.json", &[("npm", &["express", "lodash"])])],
        )
    );
}

#[test]
fn deps_are_sorted() {
    let log = log(
        r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"zebra"},{"depName":"alpha"},{"depName":"moose"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(
        result.files["package.json"]["npm"],
        vec!["alpha", "moose", "zebra"]
    );
}

#[test]
fn filters_skip_reasons() {
    let log = log(
        r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"keep"},{"depName":"bad1","skipReason":"contains-variable"},{"depName":"bad2","skipReason":"invalid-value"},{"depName":"bad3","skipReason":"invalid-version"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(result.files["package.json"]["npm"], vec!["keep"]);
}

#[test]
fn other_skip_reasons_are_kept() {
    let log = log(
        r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"pinned","skipReason":"pinned-major-version"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(result.files["package.json"]["npm"], vec!["pinned"]);
}

#[test]
fn extracts_deps_tolerate_conflicting_metadata_for_same_dep_name() {
    let log = log(
        r#"{"gomod":[{"packageFile":"go.mod","deps":[{"depName":"go","packageName":"go","datasource":"golang-version"}]}],"mise":[{"packageFile":"mise.toml","deps":[{"depName":"go","packageName":"go","datasource":"core"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(result.files["go.mod"]["gomod"], vec!["go"]);
    assert_eq!(result.files["mise.toml"]["mise"], vec!["go"]);
    assert_eq!(result.meta["go"].package_name.as_deref(), Some("go"));
    assert_eq!(result.meta["go"].datasource, None);
}

#[test]
fn extracts_extended_dep_metadata_from_lookup_logs() {
    let log = log_current(
        r#"{"mise":[{"packageFile":"mise.toml","deps":[{"depName":"biome","packageName":"biomejs/biome","datasource":"github-tags","currentValue":"2.4.12","currentVersion":"@biomejs/biome@2.4.12","extractVersion":"^v?(?<version>.+)"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    let meta = &result.meta["biome"];

    assert_eq!(meta.package_name.as_deref(), Some("biomejs/biome"));
    assert_eq!(meta.datasource.as_deref(), Some("github-tags"));
    assert_eq!(meta.current_value.as_deref(), Some("2.4.12"));
    assert_eq!(
        meta.current_version.as_deref(),
        Some("@biomejs/biome@2.4.12")
    );
    assert_eq!(meta.extract_version.as_deref(), Some("^v?(?<version>.+)"));
}

#[test]
fn extracts_legacy_manager_names_using_canonical_snapshot_keys() {
    let log = log(
        r#"{"renovate-config-presets":[{"packageFile":".github/renovate.json5","deps":[{"depName":"grafana/flint"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();

    assert_eq!(
        result.files[".github/renovate.json5"]["renovate-config"],
        vec!["grafana/flint"]
    );
}

#[test]
fn excludes_managers() {
    let log = log(
        r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"express"}]}],"cargo":[{"packageFile":"Cargo.toml","deps":[{"depName":"tokio"}]}]}"#,
    );
    let result = extract_deps(&log, &["npm".to_string()]).unwrap();
    assert!(!result.files.contains_key("package.json"));
    assert_eq!(result.files["Cargo.toml"]["cargo"], vec!["tokio"]);
}

#[test]
fn skips_deps_without_dep_name() {
    let log = log(
        r#"{"npm":[{"packageFile":"package.json","deps":[{"version":"1.0.0"},{"depName":"valid"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(result.files["package.json"]["npm"], vec!["valid"]);
}

#[test]
fn last_package_files_message_wins() {
    let bytes = format!(
            "{}\n{}\n",
            r#"{"msg":"Extracted dependencies","packageFiles":{"npm":[{"packageFile":"a.json","deps":[{"depName":"old"}]}]}}"#,
            r#"{"msg":"Extracted dependencies","packageFiles":{"npm":[{"packageFile":"b.json","deps":[{"depName":"new"}]}]}}"#,
        )
        .into_bytes();
    let result = extract_deps(&bytes, &[]).unwrap();
    assert!(
        !result.files.contains_key("a.json"),
        "should use last entry"
    );
    assert!(result.files.contains_key("b.json"));
}

#[test]
fn non_json_lines_are_skipped() {
    let bytes =
            b"not json\n{\"msg\":\"Extracted dependencies\",\"packageFiles\":{\"npm\":[{\"packageFile\":\"p.json\",\"deps\":[{\"depName\":\"x\"}]}]}}\nmore garbage\n";
    let result = extract_deps(bytes, &[]).unwrap();
    assert!(result.files.contains_key("p.json"));
}

#[test]
fn missing_message_returns_error() {
    let bytes = b"{\"msg\":\"something else\"}\n";
    let err = extract_deps(bytes, &[]).unwrap_err();
    assert!(err.to_string().contains("none of"));
    assert!(err.to_string().contains("Extracted dependencies"));
}

#[test]
fn write_and_read_snapshot_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.json");
    let deps = snapshot(
        &[
            ("serde", None, None),
            ("tokio", None, None),
            ("express", None, None),
            ("lodash", None, None),
        ],
        &[
            ("Cargo.toml", &[("cargo", &["serde", "tokio"])]),
            ("package.json", &[("npm", &["express", "lodash"])]),
        ],
    );
    write_snapshot(&path, &deps).unwrap();
    let read_back = read_snapshot(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(deps, read_back);
}

#[test]
fn reads_legacy_snapshot_format() {
    let legacy = r#"{
  "package.json": {
    "npm": [
      "express"
    ]
  }
}
"#;
    let snapshot = read_snapshot(legacy).unwrap();
    assert!(snapshot.meta.is_empty());
    assert_eq!(
        snapshot.files,
        dep_files(&[("package.json", &[("npm", &["express"])])])
    );
}

#[test]
fn write_snapshot_ends_with_newline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.json");
    write_snapshot(&path, &Snapshot::default()).unwrap();
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.ends_with('\n'));
}

#[test]
fn merge_missing_meta_from_committed_keeps_existing_details() {
    let mut generated = snapshot(
        &[("actionlint", None, Some("github-releases"))],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );
    let committed = snapshot(
        &[(
            "actionlint",
            Some("rhysd/actionlint"),
            Some("github-releases"),
        )],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );

    merge_missing_meta_from_committed(&mut generated, &committed);

    assert_eq!(
        generated.meta["actionlint"].package_name.as_deref(),
        Some("rhysd/actionlint")
    );
    assert_eq!(
        generated.meta["actionlint"].datasource.as_deref(),
        Some("github-releases")
    );
}

#[test]
fn maybe_reuse_committed_meta_merges_missing_fields() {
    let mut generated = snapshot(
        &[("actionlint", None, Some("github-releases"))],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );
    let committed = snapshot(
        &[(
            "actionlint",
            Some("rhysd/actionlint"),
            Some("github-releases"),
        )],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );

    maybe_reuse_committed_meta(&mut generated, Some(&committed));

    assert_eq!(
        generated.meta["actionlint"].package_name.as_deref(),
        Some("rhysd/actionlint")
    );
}

#[test]
fn incomplete_meta_for_rules_passes_when_meta_is_complete() {
    let snap = snapshot(
        &[(
            "actionlint",
            Some("rhysd/actionlint"),
            Some("github-releases"),
        )],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["actionlint".to_string()])),
    }];
    assert!(incomplete_meta_for_rules(&snap, &rules).is_none());
}

#[test]
fn incomplete_meta_for_rules_dep_name_rule_tolerates_missing_datasource() {
    // matchDepNames doesn't need datasource — Renovate doesn't always surface
    // one for bare-key mise tools (e.g. biome) and grouping isn't affected.
    let snap = snapshot(
        &[("biome", Some("biome"), None)],
        &[("mise.toml", &[("mise", &["biome"])])],
    );
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["biome".to_string()])),
    }];
    assert!(incomplete_meta_for_rules(&snap, &rules).is_none());
}

#[test]
fn incomplete_meta_for_rules_dep_name_rule_flags_missing_packagename() {
    let snap = snapshot(
        &[("actionlint", None, Some("github-releases"))],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["actionlint".to_string()])),
    }];
    let reason = incomplete_meta_for_rules(&snap, &rules).unwrap();
    assert!(reason.contains("actionlint"));
    assert!(reason.contains("packageName"));
}

#[test]
fn incomplete_meta_for_rules_package_name_rule_requires_datasource() {
    let snap = snapshot(
        &[("mise", Some("jdx/mise"), None)],
        &[("mise.toml", &[("mise", &["mise"])])],
    );
    let rules = vec![ComparablePackageRule {
        label: "group \"mise\"".to_string(),
        matcher: RuleMatcher::PackageNames(BTreeSet::from(["jdx/mise".to_string()])),
    }];
    let reason = incomplete_meta_for_rules(&snap, &rules).unwrap();
    assert!(reason.contains("mise"));
    assert!(reason.contains("datasource"));
}

#[test]
fn validate_extract_version_consistency_accepts_matching_extraction() {
    let snap = Snapshot {
        meta: [(
            "actionlint".to_string(),
            DepMeta {
                package_name: Some("rhysd/actionlint".to_string()),
                datasource: Some("github-releases".to_string()),
                current_value: Some("1.7.7".to_string()),
                current_version: Some("v1.7.7".to_string()),
                extract_version: Some("^v(?<version>\\S+)".to_string()),
            },
        )]
        .into_iter()
        .collect(),
        files: dep_files(&[("mise.toml", &[("mise", &["actionlint"])])]),
    };

    assert!(validate_extract_version_consistency(&snap).is_ok());
}

#[test]
fn validate_extract_version_consistency_accepts_normalized_current_version() {
    let snap = Snapshot {
        meta: [(
            "actionlint".to_string(),
            DepMeta {
                package_name: Some("rhysd/actionlint".to_string()),
                datasource: Some("github-releases".to_string()),
                current_value: Some("1.7.12".to_string()),
                current_version: Some("1.7.12".to_string()),
                extract_version: Some("^v(?<version>\\S+)".to_string()),
            },
        )]
        .into_iter()
        .collect(),
        files: dep_files(&[("mise.toml", &[("mise", &["actionlint"])])]),
    };

    assert!(validate_extract_version_consistency(&snap).is_ok());
}

#[test]
fn validate_extract_version_consistency_accepts_normalized_prefixed_current_value() {
    let snap = Snapshot {
        meta: [(
            "shellcheck".to_string(),
            DepMeta {
                package_name: Some("koalaman/shellcheck".to_string()),
                datasource: Some("github-releases".to_string()),
                current_value: Some("v0.11.0".to_string()),
                current_version: Some("0.11.0".to_string()),
                extract_version: Some("^v(?<version>\\S+)".to_string()),
            },
        )]
        .into_iter()
        .collect(),
        files: dep_files(&[("mise.toml", &[("mise", &["shellcheck"])])]),
    };

    assert!(validate_extract_version_consistency(&snap).is_ok());
}

#[test]
fn validate_extract_version_consistency_flags_mismatch() {
    let snap = Snapshot {
        meta: [(
            "biome".to_string(),
            DepMeta {
                package_name: Some("biomejs/biome".to_string()),
                datasource: Some("github-tags".to_string()),
                current_value: Some("2.4.12".to_string()),
                current_version: Some("@biomejs/biome@2.4.12".to_string()),
                extract_version: Some("^v?(?<version>.+)".to_string()),
            },
        )]
        .into_iter()
        .collect(),
        files: dep_files(&[("mise.toml", &[("mise", &["biome"])])]),
    };

    let err = validate_extract_version_consistency(&snap).unwrap_err();
    let msg = err.to_string();

    assert!(msg.contains("biome"));
    assert!(msg.contains("@biomejs/biome@2.4.12"));
    assert!(msg.contains("^v?(?<version>.+)"));
    assert!(msg.contains("2.4.12"));
}

#[test]
fn validate_extract_version_consistency_flags_no_match() {
    let snap = Snapshot {
        meta: [(
            "biome".to_string(),
            DepMeta {
                package_name: Some("biomejs/biome".to_string()),
                datasource: Some("github-tags".to_string()),
                current_value: Some("2.4.12".to_string()),
                current_version: Some("@biomejs/biome@2.4.12".to_string()),
                extract_version: Some("^v(?<version>.+)$".to_string()),
            },
        )]
        .into_iter()
        .collect(),
        files: dep_files(&[("mise.toml", &[("mise", &["biome"])])]),
    };

    let err = validate_extract_version_consistency(&snap).unwrap_err();
    let msg = err.to_string();

    assert!(msg.contains("no match"), "unexpected error:\n{msg}");
    assert!(msg.contains("^v(?<version>.+)$"));
}

#[test]
fn patch_semver_equivalent_mise_values_rewrites_to_preferred_shape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mise.toml");
    std::fs::write(&path, "[tools]\nprotoc = \"35.0\"\n").unwrap();

    let snap = Snapshot {
        meta: [(
            "protoc".to_string(),
            DepMeta {
                package_name: Some("protocolbuffers/protobuf".to_string()),
                datasource: Some("github-releases".to_string()),
                current_value: Some("35.0".to_string()),
                current_version: Some("v35".to_string()),
                extract_version: Some("^v(?<version>\\S+)".to_string()),
            },
        )]
        .into_iter()
        .collect(),
        files: dep_files(&[("mise.toml", &[("mise", &["protoc"])])]),
    };
    let mismatches = extract_version_mismatches(&snap).unwrap();

    let changed = patch_semver_equivalent_mise_values(dir.path(), &snap, &mismatches).unwrap();

    assert!(changed);
    let result = std::fs::read_to_string(path).unwrap();
    assert!(
        result.contains("protoc = \"35\""),
        "rewritten content: {result}"
    );
}

#[test]
fn patch_extract_version_overrides_appends_rule() {
    let tmp = write_tmp("{\n  extends: [\"config:recommended\"]\n}\n");
    let changed = patch_extract_version_overrides(
        tmp.path(),
        &[ExtractVersionMismatch {
            dep_name: "biome".to_string(),
            package_name: Some("biomejs/biome".to_string()),
            current_value: "2.4.12".to_string(),
            current_version: "@biomejs/biome@2.4.12".to_string(),
            extract_version: "^v?(?<version>.+)".to_string(),
            extracted_value: Some("@biomejs/biome@2.4.12".to_string()),
            suggested_extract_version: Some("^@biomejs/biome@(?<version>.+)$".to_string()),
        }],
    )
    .unwrap();

    assert!(changed);

    let parsed: serde_json::Value =
        json5::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
    let rules = parsed["packageRules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["matchDepNames"][0], "biome");
    assert_eq!(
        rules[0]["extractVersion"],
        "^@biomejs/biome@(?<version>.+)$"
    );
}

#[test]
fn patch_extract_version_overrides_preserves_json5_formatting() {
    let tmp = write_tmp(
        r#"{
  // keep this comment
  extends: ["config:recommended"],
}
"#,
    );
    let changed = patch_extract_version_overrides(
        tmp.path(),
        &[ExtractVersionMismatch {
            dep_name: "biome".to_string(),
            package_name: Some("biomejs/biome".to_string()),
            current_value: "2.4.12".to_string(),
            current_version: "@biomejs/biome@2.4.12".to_string(),
            extract_version: "^v?(?<version>.+)".to_string(),
            extracted_value: Some("@biomejs/biome@2.4.12".to_string()),
            suggested_extract_version: Some("^@biomejs/biome@(?<version>.+)$".to_string()),
        }],
    )
    .unwrap();

    assert!(changed);

    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(result.contains("// keep this comment"));
    assert!(result.contains("extends: [\"config:recommended\"]"));

    let parsed: serde_json::Value = json5::from_str(&result).unwrap();
    let rules = parsed["packageRules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["matchDepNames"][0], "biome");
}

#[test]
fn validate_rule_coverage_flags_split_dep_names_for_same_package() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("renovate.json5");
    std::fs::write(
        &config_path,
        r#"{
  packageRules: [
    {
      groupName: "linters",
      matchDepNames: ["actionlint"]
    }
  ]
}
"#,
    )
    .unwrap();
    let snapshot = snapshot(
        &[
            (
                "actionlint",
                Some("rhysd/actionlint"),
                Some("github-releases"),
            ),
            (
                "rhysd/actionlint",
                Some("rhysd/actionlint"),
                Some("github-releases"),
            ),
        ],
        &[
            ("mise.toml", &[("mise", &["actionlint"])]),
            ("README.md", &[("regex", &["rhysd/actionlint"])]),
        ],
    );

    let parsed = comparable_package_rules_for_config(&config_path).unwrap();
    let err = validate_rule_coverage(&snapshot, &parsed.rules).unwrap_err();
    let msg = err.to_string();

    assert!(msg.contains("rhysd/actionlint"));
    assert!(msg.contains("matched [actionlint]"));
    assert!(msg.contains("unmatched [rhysd/actionlint]"));
}

#[test]
fn comparable_rules_reject_non_string_match_dep_names() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("renovate.json5");
    std::fs::write(
        &config_path,
        r#"{
  packageRules: [
    {
      groupName: "linters",
      matchDepNames: ["actionlint", 42]
    }
  ]
}
"#,
    )
    .unwrap();

    let err = comparable_package_rules_for_config(&config_path).unwrap_err();

    assert!(
        err.to_string()
            .contains("package rule index 0 must declare matchDepNames[1] as a string")
    );
}

#[test]
fn comparable_rules_reject_non_string_match_package_names() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("renovate.json5");
    std::fs::write(
        &config_path,
        r#"{
  packageRules: [
    {
      description: "packages",
      matchPackageNames: [false]
    }
  ]
}
"#,
    )
    .unwrap();

    let err = comparable_package_rules_for_config(&config_path).unwrap_err();

    assert!(
        err.to_string()
            .contains("package rule index 0 must declare matchPackageNames[0] as a string")
    );
}

#[test]
fn comparable_rules_reject_additional_match_constraints() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("renovate.json5");
    std::fs::write(
        &config_path,
        r#"{
  packageRules: [
    {
      groupName: "linters",
      matchDepNames: ["actionlint"],
      matchManagers: ["custom.regex"]
    }
  ]
}
"#,
    )
    .unwrap();

    let parsed = comparable_package_rules_for_config(&config_path).unwrap();

    assert!(parsed.rules.is_empty());
    assert_eq!(parsed.skipped_notes.len(), 1);
    assert!(parsed.skipped_notes[0].contains("group \"linters\""));
    assert!(parsed.skipped_notes[0].contains("matchManagers"));
    assert!(parsed.skipped_notes[0].contains("skipped package rule"));
}

#[test]
fn comparable_rules_allow_non_contextual_match_constraints() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("renovate.json5");
    std::fs::write(
        &config_path,
        r#"{
  packageRules: [
    {
      description: "slim tags",
      matchPackageNames: ["jdx/mise"],
      matchCurrentValue: "/^slim-/"
    }
  ]
}
"#,
    )
    .unwrap();

    let parsed = comparable_package_rules_for_config(&config_path).unwrap();

    assert_eq!(parsed.rules.len(), 1);
    assert!(parsed.skipped_notes.is_empty());
}

#[test]
fn notes_output_formats_skipped_rule_messages() {
    let out = notes_output(&[
        "first skipped note".to_string(),
        "second skipped note".to_string(),
    ]);

    assert_eq!(out, "first skipped note\nsecond skipped note\n");
}

#[test]
fn trim_snapshot_meta_keeps_only_rule_relevant_deps() {
    let snapshot = snapshot(
        &[
            (
                "actionlint",
                Some("rhysd/actionlint"),
                Some("github-releases"),
            ),
            (
                "rhysd/actionlint",
                Some("rhysd/actionlint"),
                Some("github-releases"),
            ),
            (
                "Swatinem/rust-cache",
                Some("Swatinem/rust-cache"),
                Some("github-tags"),
            ),
        ],
        &[
            ("mise.toml", &[("mise", &["actionlint"])]),
            (
                "src/init/scaffold.rs",
                &[("regex", &["Swatinem/rust-cache"])],
            ),
            ("README.md", &[("regex", &["rhysd/actionlint"])]),
        ],
    );
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["actionlint".to_string()])),
    }];

    let relevant = relevant_dep_names(&snapshot, &rules);

    assert!(relevant.contains("actionlint"));
    assert!(relevant.contains("rhysd/actionlint"));
    assert!(!relevant.contains("Swatinem/rust-cache"));
}

#[test]
fn unified_diff_contains_added_and_removed_lines() {
    let old = snapshot(
        &[("old-dep", None, None)],
        &[("a.json", &[("npm", &["old-dep"])])],
    );
    let new = snapshot(
        &[("new-dep", None, None)],
        &[("a.json", &[("npm", &["new-dep"])])],
    );
    let diff = unified_diff(&old, &new, ".github/renovate-tracked-deps.json");
    assert!(diff.contains("-"), "should have removals");
    assert!(diff.contains("+"), "should have additions");
    assert!(diff.contains("old-dep"));
    assert!(diff.contains("new-dep"));
}

#[test]
fn unified_diff_header_uses_display_path() {
    let old = snapshot(&[("x", None, None)], &[("a.json", &[("npm", &["x"])])]);
    let new = snapshot(&[("y", None, None)], &[("a.json", &[("npm", &["y"])])]);
    let diff = unified_diff(&old, &new, "renovate-tracked-deps.json");
    assert!(diff.contains("renovate-tracked-deps.json"));
}

#[test]
fn display_path_normalizes_separators() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join(".github")
        .join("renovate-tracked-deps.json");
    assert_eq!(
        display_path(dir.path(), &path),
        ".github/renovate-tracked-deps.json"
    );
}

#[test]
fn resolves_supported_renovate_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join(".renovaterc.json");
    std::fs::write(&config_path, "{}\n").unwrap();

    let resolved = resolve_renovate_config_path(dir.path()).unwrap();

    assert_eq!(resolved, config_path);
}

#[test]
fn missing_supported_renovate_config_file_returns_error() {
    let dir = tempfile::tempdir().unwrap();

    let err = resolve_renovate_config_path(dir.path()).unwrap_err();
    let msg = err.to_string();

    assert!(msg.contains("no supported Renovate config file found"));
    assert!(
        RENOVATE_CONFIG_PATTERNS
            .iter()
            .all(|path| msg.contains(path))
    );
}

#[test]
fn committed_path_uses_same_dir_as_found_config() {
    assert_eq!(
        committed_path_for_config(Path::new("renovate.json5")),
        PathBuf::from("renovate-tracked-deps.json")
    );
    assert_eq!(
        committed_path_for_config(Path::new(".github/renovate.json5")),
        PathBuf::from(".github/renovate-tracked-deps.json")
    );
}

fn file_list(paths: &[&str], full: bool) -> FileList {
    FileList {
        files: paths.iter().map(PathBuf::from).collect(),
        changed_paths: paths.iter().map(|path| path.to_string()).collect(),
        merge_base: Some("base".to_string()),
        full,
    }
}

#[test]
fn relevant_when_full_mode() {
    let dir = tempfile::tempdir().unwrap();
    assert!(is_relevant(&file_list(&[], true), dir.path()));
}

#[test]
fn relevant_when_renovate_config_changed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(is_relevant(
        &file_list(
            &[dir.path().join(".github/renovate.json5").to_str().unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn relevant_when_snapshot_changed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate-tracked-deps.json"),
        "{}\n",
    )
    .unwrap();

    assert!(is_relevant(
        &file_list(
            &[dir
                .path()
                .join(".github/renovate-tracked-deps.json")
                .to_str()
                .unwrap()],
            false
        ),
        dir.path()
    ));
}

#[test]
fn relevant_when_tracked_manifest_changed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    assert!(is_relevant(
        &file_list(&[dir.path().join("package.json").to_str().unwrap()], false),
        dir.path()
    ));
}

#[test]
fn relevant_when_tracked_manifest_was_deleted() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    let file_list = FileList {
        files: vec![],
        changed_paths: vec!["package.json".to_string()],
        merge_base: Some("base".to_string()),
        full: false,
    };

    assert!(is_relevant(&file_list, dir.path()));
}

#[test]
fn not_relevant_for_untracked_change() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    write_snapshot(
        &dir.path().join(".github/renovate-tracked-deps.json"),
        &snapshot(
            &[("express", None, None)],
            &[("package.json", &[("npm", &["express"])])],
        ),
    )
    .unwrap();

    assert!(!is_relevant(
        &file_list(&[dir.path().join("README.md").to_str().unwrap()], false),
        dir.path()
    ));
}

#[test]
fn relevant_when_snapshot_is_unparsable() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/renovate-tracked-deps.json"),
        "{not json}\n",
    )
    .unwrap();

    assert!(is_relevant(
        &file_list(&[dir.path().join("README.md").to_str().unwrap()], false),
        dir.path()
    ));
}

#[test]
fn extract_failure_snippet_prefers_error_lines() {
    let log = "\
{\"level\":20,\"msg\":\"Parsing configs\"}\n\
{\"level\":30,\"msg\":\"Renovate started\"}\n\
{\"level\":50,\"msg\":\"Failed\",\"err\":{\"message\":\"boom\"}}\n\
{\"level\":20,\"msg\":\"trailing debug\"}\n";
    let snippet = extract_failure_snippet(log);
    assert_eq!(snippet, "level=50 Failed: boom");
}

#[test]
fn extract_failure_snippet_handles_missing_msg() {
    let log = "\
{\"level\":50,\"err\":{\"message\":\"boom\"}}\n\
{\"level\":60,\"msg\":\"\",\"err\":{\"message\":\"fatal\"}}\n\
{\"level\":40,\"msg\":\"warn only\"}\n";
    let snippet = extract_failure_snippet(log);
    assert_eq!(snippet, "level=50 boom\nlevel=60 fatal\nlevel=40 warn only");
}

#[test]
fn extract_failure_snippet_falls_back_to_tail() {
    let mut log = String::new();
    for i in 0..30 {
        log.push_str(&format!("{{\"level\":20,\"msg\":\"line {i}\"}}\n"));
    }
    let snippet = extract_failure_snippet(&log);
    let lines: Vec<&str> = snippet.lines().collect();
    assert_eq!(lines.len(), 20);
    assert!(lines.last().unwrap().contains("line 29"));
    assert!(lines.first().unwrap().contains("line 10"));
}
