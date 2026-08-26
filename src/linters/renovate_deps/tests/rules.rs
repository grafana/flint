use super::*;

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
        has_extract_version: false,
    }];
    assert!(incomplete_meta_for_rules(&snap, &rules).is_none());
}

#[test]
fn version_validation_uses_lookup_for_new_rule_relevant_dependency() {
    let generated = snapshot(
        &[(
            "checkstyle",
            Some("checkstyle/checkstyle"),
            Some("github-tags"),
        )],
        &[("mise.toml", &[("mise", &["checkstyle"])])],
    );
    let committed = Snapshot::default();
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["checkstyle".to_string()])),
        has_extract_version: false,
    }];

    assert!(version_validation_needs_lookup(
        &generated,
        Some(&committed),
        &rules,
        &HashSet::new()
    ));
}

#[test]
fn bundled_preset_exposes_dependencies_with_extract_version_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let deps = bundled_extract_version_dep_names(
        dir.path(),
        r#"{ extends: ["config:recommended", "github>grafana/flint#v1.2.3"] }"#,
    );

    assert_eq!(
        deps,
        HashSet::from(["biome".to_string(), "checkstyle".to_string()])
    );
    assert!(bundled_extract_version_dep_names(dir.path(), "{}").is_empty());
}

#[test]
fn bundled_preset_is_resolved_through_local_preset() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("renovate-preset.json5"),
        r#"{ extends: ["github>grafana/flint"] }"#,
    )
    .unwrap();

    let deps =
        bundled_extract_version_dep_names(dir.path(), r#"{ extends: ["local>renovate-preset"] }"#);

    assert_eq!(
        deps,
        HashSet::from(["biome".to_string(), "checkstyle".to_string()])
    );
}

#[test]
fn version_validation_uses_lookup_when_dependency_identity_changes() {
    let generated = snapshot(
        &[(
            "checkstyle",
            Some("checkstyle/checkstyle"),
            Some("github-tags"),
        )],
        &[("mise.toml", &[("mise", &["checkstyle"])])],
    );
    let committed = snapshot(
        &[(
            "checkstyle",
            Some("checkstyle/checkstyle"),
            Some("github-releases"),
        )],
        &[("mise.toml", &[("mise", &["checkstyle"])])],
    );
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["checkstyle".to_string()])),
        has_extract_version: false,
    }];

    assert!(version_validation_needs_lookup(
        &generated,
        Some(&committed),
        &rules,
        &HashSet::new()
    ));
}

#[test]
fn version_validation_keeps_extract_for_stable_dependency_identity() {
    let generated = snapshot(
        &[(
            "checkstyle",
            Some("checkstyle/checkstyle"),
            Some("github-tags"),
        )],
        &[("mise.toml", &[("mise", &["checkstyle"])])],
    );
    let committed = generated.clone();
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["checkstyle".to_string()])),
        has_extract_version: false,
    }];

    assert!(!version_validation_needs_lookup(
        &generated,
        Some(&committed),
        &rules,
        &HashSet::new()
    ));
}

#[test]
fn version_validation_uses_lookup_for_new_package_name_rule_candidate() {
    let generated = snapshot(&[], &[("mise.toml", &[("mise", &["actionlint"])])]);
    let rules = vec![ComparablePackageRule {
        label: "group \"linters\"".to_string(),
        matcher: RuleMatcher::PackageNames(BTreeSet::from(["rhysd/actionlint".to_string()])),
        has_extract_version: false,
    }];

    assert!(version_validation_needs_lookup(
        &generated,
        Some(&Snapshot::default()),
        &rules,
        &HashSet::new()
    ));
}

#[test]
fn version_validation_uses_lookup_for_incomplete_extract_version_rule_metadata() {
    let generated = snapshot(
        &[(
            "checkstyle",
            Some("checkstyle/checkstyle"),
            Some("github-releases"),
        )],
        &[("mise.toml", &[("mise", &["checkstyle"])])],
    );
    let committed = generated.clone();
    let rules = vec![ComparablePackageRule {
        label: "extract checkstyle version".to_string(),
        matcher: RuleMatcher::DepNames(BTreeSet::from(["checkstyle".to_string()])),
        has_extract_version: true,
    }];

    assert!(version_validation_needs_lookup(
        &generated,
        Some(&committed),
        &rules,
        &HashSet::new()
    ));
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
        has_extract_version: false,
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
        has_extract_version: false,
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
        has_extract_version: false,
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
        action_meta: BTreeMap::new(),
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
        action_meta: BTreeMap::new(),
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
        action_meta: BTreeMap::new(),
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
        action_meta: BTreeMap::new(),
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
        action_meta: BTreeMap::new(),
        files: dep_files(&[("mise.toml", &[("mise", &["biome"])])]),
    };

    let err = validate_extract_version_consistency(&snap).unwrap_err();
    let msg = err.to_string();

    assert!(msg.contains("no match"), "unexpected error:\n{msg}");
    assert!(msg.contains("^v(?<version>.+)$"));
}

#[test]
fn equivalent_version_shapes_accepts_four_part_versions() {
    assert!(equivalent_version_shapes("1.2.3.4", "v1.2.3.4"));
    assert!(equivalent_version_shapes("1.2.3", "1.2.3.0"));
    assert!(!equivalent_version_shapes("1.2.3.4", "1.2.3.5"));
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
        action_meta: BTreeMap::new(),
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
fn add_to_package_rules_reuses_existing_trailing_comma() {
    let content = r#"{
  packageRules: [
    {
      matchDepNames: ["renovate"],
    },
  ],
}
"#;
    let rule = serde_json::json!({
        "description": "Flint autofix",
        "matchDepNames": ["checkstyle"],
        "extractVersion": "^checkstyle-(?<version>.+)$",
    });

    let updated = add_to_package_rules(content, &[rule]).unwrap();

    assert!(!updated.contains("},,"), "updated config: {updated}");
    let parsed: serde_json::Value = json5::from_str(&updated).unwrap();
    assert_eq!(parsed["packageRules"].as_array().unwrap().len(), 2);
}

#[test]
fn add_to_package_rules_puts_comma_before_trailing_line_comment() {
    let content = r#"{
  packageRules: [
    {
      matchDepNames: ["renovate"]
    } // keep this comment
  ],
}
"#;
    let rule = serde_json::json!({
        "description": "Flint autofix",
        "matchDepNames": ["checkstyle"],
        "extractVersion": "^checkstyle-(?<version>.+)$",
    });

    let updated = add_to_package_rules(content, &[rule]).unwrap();

    assert!(
        updated.contains("}, // keep this comment"),
        "updated config: {updated}"
    );
    let parsed: serde_json::Value = json5::from_str(&updated).unwrap();
    assert_eq!(parsed["packageRules"].as_array().unwrap().len(), 2);
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
        has_extract_version: false,
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
fn unified_diff_ignores_dep_order_only_changes() {
    let old = snapshot(
        &[
            ("alpha", None, None),
            ("moose", None, None),
            ("zebra", None, None),
        ],
        &[("package.json", &[("npm", &["zebra", "alpha", "moose"])])],
    );
    let new = snapshot(
        &[
            ("alpha", None, None),
            ("moose", None, None),
            ("zebra", None, None),
        ],
        &[("package.json", &[("npm", &["moose", "zebra", "alpha"])])],
    );

    let diff = unified_diff(&old, &new, ".github/renovate-tracked-deps.json");

    assert!(
        diff.is_empty(),
        "ordering-only changes should not diff: {diff}"
    );
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
