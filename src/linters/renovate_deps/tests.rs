use super::install_patch::configure_extract_workaround_env;
use super::rules::{ComparablePackageRule, RuleMatcher, needs_metadata_lookup, relevant_dep_names};
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
    let changed = configure_renovate_deps_config(tmp.path(), None).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(result.contains("[checks.renovate-deps]"));
    assert!(result.contains("# exclude_managers = []"));
}

#[test]
fn configure_renovate_deps_appends_migrated_managers() {
    let tmp = write_tmp("[settings]\n");
    let managers = vec!["github-actions".to_string(), "cargo".to_string()];
    let changed = configure_renovate_deps_config(tmp.path(), Some(&managers)).unwrap();
    assert!(changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(
        result.contains("exclude_managers = [\"github-actions\", \"cargo\"]"),
        "managers written uncommented: {result}"
    );
    assert!(!result.contains("# exclude_managers"));
}

#[test]
fn configure_renovate_deps_keeps_existing_managers() {
    let tmp = write_tmp("[checks.renovate-deps]\nexclude_managers = [\"npm\"]\n");
    let managers = vec!["github-actions".to_string(), "cargo".to_string()];
    let changed = configure_renovate_deps_config(tmp.path(), Some(&managers)).unwrap();
    assert!(!changed);
    let result = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(result.contains("exclude_managers = [\"npm\"]"));
    assert!(!result.contains("github-actions"));
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
fn maybe_reuse_committed_meta_merges_when_refresh_meta_is_disabled() {
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

    maybe_reuse_committed_meta(&mut generated, Some(&committed), false);

    assert_eq!(
        generated.meta["actionlint"].package_name.as_deref(),
        Some("rhysd/actionlint")
    );
}

#[test]
fn maybe_reuse_committed_meta_skips_merge_when_refresh_meta_is_enabled() {
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

    maybe_reuse_committed_meta(&mut generated, Some(&committed), true);

    assert_eq!(generated.meta["actionlint"].package_name, None);
}

#[test]
fn metadata_lookup_not_needed_without_comparable_rules() {
    let generated = snapshot(
        &[("actionlint", None, Some("github-releases"))],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );

    assert!(!needs_metadata_lookup(&generated, &[]));
}

#[test]
fn metadata_lookup_needed_for_dep_name_rule_when_package_name_missing() {
    let generated = snapshot(
        &[("actionlint", None, Some("github-releases"))],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["actionlint".to_string()])),
    }];

    assert!(needs_metadata_lookup(&generated, &rules));
}

#[test]
fn metadata_lookup_not_needed_for_dep_name_rule_when_cached_package_name_present() {
    let generated = snapshot(
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

    assert!(!needs_metadata_lookup(&generated, &rules));
}

#[test]
fn metadata_lookup_needed_for_package_name_rule_when_any_extracted_dep_lacks_package_name() {
    let generated = snapshot(
        &[("actionlint", None, Some("github-releases"))],
        &[("mise.toml", &[("mise", &["actionlint"])])],
    );
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::PackageNames(BTreeSet::from(["rhysd/actionlint".to_string()])),
    }];

    assert!(needs_metadata_lookup(&generated, &rules));
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
      matchPackageNames: ["ghcr.io/super-linter/super-linter"],
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
fn relevant_when_snapshot_is_unparseable() {
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
