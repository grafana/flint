use super::*;

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
        r#"{"npm":[{"packageFile":"package.json","deps":[{"depName":"keep"},{"depName":"bad1","skipReason":"contains-variable"},{"depName":"bad2","skipReason":"invalid-value"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(result.files["package.json"]["npm"], vec!["bad2", "keep"]);
}

#[test]
fn invalid_lookup_skip_reasons_are_kept_for_all_managers() {
    for manager in ["npm", "mise", "docker", "github-actions"] {
        for skip_reason in ["invalid-value", "invalid-version"] {
            let log = log(&format!(
                r#"{{"{manager}":[{{"packageFile":"deps.txt","deps":[{{"depName":"declared","skipReason":"{skip_reason}"}}]}}]}}"#
            ));
            let result = extract_deps(&log, &[]).unwrap();
            assert_eq!(
                result.files["deps.txt"][manager],
                vec!["declared"],
                "{skip_reason} should remain tracked for {manager}"
            );
        }
    }
}

#[test]
fn invalid_version_is_kept() {
    // Lookup-time versioning failures (e.g. java-jdk `temurin-*` currentVersion
    // rejected by workarounds:javaLTSVersions regex) must not drop the dep.
    let log = log(
        r#"{"mise":[{"packageFile":"mise.toml","deps":[{"depName":"java","skipReason":"invalid-version"}]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    assert_eq!(result.files["mise.toml"]["mise"], vec!["java"]);
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
fn extracts_action_metadata_per_monorepo_target() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[
          {"depName":"grafana/shared-workflows","packageName":"grafana/shared-workflows","depType":"action","currentValue":"create-github-app-token/v0.2.2","replaceString":"grafana/shared-workflows/actions/create-github-app-token@ae92934a14a48b94494dbc06d74a81d47fe08a40 # create-github-app-token/v0.2.2"},
          {"depName":"grafana/shared-workflows","packageName":"grafana/shared-workflows","depType":"action","currentValue":"main","replaceString":"grafana/shared-workflows/.github/workflows/sign-and-attest.yml@abc1234567890123456789012345678901234567 # main"}
        ]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();

    assert_eq!(result.action_meta.len(), 2);
    assert_eq!(
        result.action_meta["grafana/shared-workflows/actions/create-github-app-token"],
        ActionMeta {
            package_name: "grafana/shared-workflows".to_string(),
            ref_kind: "version-tag".to_string(),
            compatibility: Some("create-github-app-token".to_string()),
            ref_: None,
        }
    );
    assert_eq!(
        result.action_meta["grafana/shared-workflows/.github/workflows/sign-and-attest.yml"]
            .ref_
            .as_deref(),
        Some("main")
    );
}

#[test]
fn action_metadata_accepts_bare_version_for_nested_action() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[
          {"depName":"example/repo","packageName":"example/repo","depType":"action","currentValue":"v0.2.2","replaceString":"example/repo/actions/nested/action@ae92934a14a48b94494dbc06d74a81d47fe08a40 # v0.2.2"}
        ]}]}"#,
    );
    assert!(
        extract_deps(&log, &[]).unwrap().action_meta["example/repo/actions/nested/action"]
            .compatibility
            .is_none()
    );
}

#[test]
fn action_metadata_uses_ref_comment_without_current_value() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[
          {"depName":"grafana/shared-workflows","packageName":"grafana/shared-workflows","depType":"action","replaceString":"grafana/shared-workflows/actions/create-github-app-token@ae92934a14a48b94494dbc06d74a81d47fe08a40 # create-github-app-token/v0.2.2"}
        ]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    let meta = &result.action_meta["grafana/shared-workflows/actions/create-github-app-token"];
    assert_eq!(meta.ref_kind, "version-tag");
    assert_eq!(
        meta.compatibility.as_deref(),
        Some("create-github-app-token")
    );
}

#[test]
fn action_metadata_accepts_bare_version_for_top_level_action() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[
          {"depName":"actions/checkout","packageName":"actions/checkout","depType":"action","currentValue":"v4.2.2","replaceString":"actions/checkout@abc1234567890123456789012345678901234567 # v4.2.2"}
        ]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    let meta = &result.action_meta["actions/checkout"];
    assert_eq!(meta.ref_kind, "version-tag");
    assert!(meta.compatibility.is_none());
}

#[test]
fn action_ref_classification_supports_major_and_prerelease_tags() {
    assert_eq!(version_tag_compatibility("v4"), Some(None));
    assert_eq!(
        version_tag_compatibility("component/v1"),
        Some(Some("component".into()))
    );
    assert_eq!(version_tag_compatibility("v1.2.3-rc.1+build.7"), Some(None));
    assert_eq!(
        version_tag_compatibility("component/v1.2.3-rc.1+build.7"),
        Some(Some("component".into()))
    );
    assert_eq!(
        version_tag_compatibility("component-v1.2.3-rc.1+build.7"),
        Some(Some("component".into()))
    );
}

#[test]
fn action_metadata_accepts_branch_refs_for_nested_actions() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[
          {"depName":"example/repo","packageName":"example/repo","depType":"action","currentValue":"main","replaceString":"example/repo/actions/nested/action@abc1234567890123456789012345678901234567 # main"}
        ]}]}"#,
    );
    let result = extract_deps(&log, &[]).unwrap();
    let meta = &result.action_meta["example/repo/actions/nested/action"];
    assert_eq!(meta.ref_kind, "branch");
    assert_eq!(meta.ref_.as_deref(), Some("main"));
}

#[test]
fn lookup_deterministic_digest_warning_is_invalid() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[{"depName":"grafana/shared-workflows","warnings":[{"message":"Could not determine new digest for update (github-digest package grafana/shared-workflows)"}]}]}]}"#,
    );
    let err = validate_lookup_action_warnings(&log, &[]).unwrap_err();
    assert!(err.to_string().contains("invalid GitHub Action ref"));

    assert!(
        validate_lookup_action_warnings(b"Could not determine new digest for update\n", &[])
            .is_ok()
    );
}

#[test]
fn lookup_no_result_warning_is_inconclusive() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[{"depName":"grafana/shared-workflows","warnings":[{"message":"Failed to look up github-tags package grafana/shared-workflows: no-result"}]}]}],"docker":[{"packageFile":"Dockerfile","deps":[{"depName":"grafana/shared-workflows","warnings":[{"message":"Could not determine new digest for update (docker package grafana/shared-workflows)"}]}]}]}"#,
    );
    assert!(validate_lookup_action_warnings(&log, &[]).is_ok());
}

#[test]
fn lookup_action_warning_respects_manager_exclusion() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[{"depName":"grafana/shared-workflows","warnings":[{"message":"Could not determine new digest for update (github-digest package grafana/shared-workflows)"}]}]}]}"#,
    );
    let exclusions = vec!["github-actions".to_string()];
    assert!(validate_lookup_action_warnings(&log, &exclusions).is_ok());
}

#[test]
fn action_metadata_conflicts_are_rejected_for_same_target() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[
          {"depName":"example/repo","packageName":"example/repo","depType":"action","currentValue":"v1.2.3","replaceString":"example/repo/actions/nested/action@abc1234567890123456789012345678901234567 # v1.2.3"},
          {"depName":"example/repo","packageName":"example/repo","depType":"action","currentValue":"main","replaceString":"example/repo/actions/nested/action@abc1234567890123456789012345678901234567 # main"}
        ]}]}"#,
    );
    let err = extract_deps(&log, &[]).unwrap_err();
    assert!(err.to_string().contains("conflicting stable metadata"));
}

#[test]
fn action_metadata_respects_manager_exclusions() {
    let log = log_current(
        r#"{"github-actions":[{"packageFile":".github/workflows/ci.yml","deps":[
          {"depName":"grafana/shared-workflows","packageName":"grafana/shared-workflows","depType":"action","currentValue":"main","replaceString":"grafana/shared-workflows/.github/workflows/sign-and-attest.yml@abc1234567890123456789012345678901234567 # main"}
        ]}]}"#,
    );
    let result = extract_deps(&log, &["github-actions".to_string()]).unwrap();
    assert!(result.action_meta.is_empty());
    assert!(result.files.is_empty());
}

#[test]
fn reads_snapshot_without_action_metadata() {
    let snapshot = read_snapshot(
        r#"{"meta":{},"files":{"workflow.yml":{"github-actions":["actions/checkout"]}}}"#,
    )
    .unwrap();
    assert!(snapshot.action_meta.is_empty());
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
