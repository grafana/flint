use super::*;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

#[test]
fn ktlint_full_runs_keep_file_list_filtering() {
    let check = builtin()
        .into_iter()
        .find(|check| check.name == "ktlint")
        .expect("ktlint registry entry");
    let crate::registry::CheckKind::Template {
        check_cmd,
        full_cmd,
        ..
    } = &check.kind
    else {
        panic!("ktlint must use a command template");
    };

    assert!(check_cmd.contains("{FILES}"));
    assert!(full_cmd.is_empty(), "ktlint must not scan the project root");
}

#[test]
fn project_wide_checks_are_explicit_file_selection_exceptions() {
    use crate::registry::FileSelection;

    let expected = [
        "cargo-clippy",
        "cargo-fmt",
        "flint-setup",
        "golangci-lint",
        "kube-linter",
        "renovate-deps",
    ];
    let mut exceptions: Vec<&str> = builtin()
        .iter()
        .filter(|check| check.file_selection == FileSelection::ProjectWide)
        .map(|check| check.name)
        .collect();
    exceptions.sort_unstable();
    assert_eq!(exceptions, expected);

    for check in builtin() {
        if let CheckKind::Template {
            full_cmd,
            full_fix_cmd,
            ..
        } = check.kind
        {
            assert!(
                (full_cmd.is_empty() && full_fix_cmd.is_empty())
                    || check.file_selection == FileSelection::ProjectWide,
                "{} has an unscoped full command without declaring a project-wide exception",
                check.name
            );
        }
    }
}

fn normalized_command_prefix(check: &Check) -> Option<String> {
    let command = match &check.kind {
        crate::registry::CheckKind::Template {
            check_cmd,
            full_cmd,
            ..
        } => {
            if !full_cmd.is_empty() {
                *full_cmd
            } else {
                *check_cmd
            }
        }
        crate::registry::CheckKind::Native(_) => return None,
    };

    let mut words = vec![];
    for token in command.split_whitespace() {
        if token.starts_with('-') || token.contains('{') {
            break;
        }
        words.push(token);
        if words.len() == 2 {
            break;
        }
    }

    (!words.is_empty()).then(|| words.join("-"))
}

/// Guardrail: check names should usually match the binary users recognize in
/// logs, config, and docs. For subcommand-style tools, a hyphenated native
/// command prefix such as `cargo-fmt` or `dotnet-format` is also acceptable.
#[test]
fn names_prefer_binary_or_native_command() {
    const ALLOWED_ALIASES: &[(&str, &str)] = &[("editorconfig-checker", "ec")];

    let violations: Vec<String> = builtin()
        .into_iter()
        .filter(|check| check.uses_binary())
        .filter(|check| !check.kind.is_native())
        .filter_map(|check| {
            let allowed = ALLOWED_ALIASES
                .iter()
                .any(|(name, bin)| check.name == *name && check.bin_name == *bin);
            let matches_command = normalized_command_prefix(&check).as_deref() == Some(check.name);
            (check.name != check.bin_name && !matches_command && !allowed).then(|| {
                format!(
                    "{} should match binary {} or native command prefix",
                    check.name, check.bin_name
                )
            })
        })
        .collect();

    assert!(
        violations.is_empty(),
        "registry check names drifted from the binary/native-command convention:\n{}",
        violations.join("\n")
    );
}

#[test]
fn case_directories_match_registry() {
    let cases_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let mut allowed: BTreeSet<String> = builtin()
        .into_iter()
        .map(|check| check.name.to_string())
        .collect();
    allowed.insert("general".to_string());

    let actual: BTreeSet<String> = std::fs::read_dir(&cases_dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", cases_dir.display()))
        .map(|entry| {
            entry.unwrap_or_else(|e| panic!("failed to read entry in {}: {e}", cases_dir.display()))
        })
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    let unexpected: Vec<String> = actual.difference(&allowed).cloned().collect();

    assert!(
        unexpected.is_empty(),
        "tests/cases contains top-level groups that are neither `general` nor registered checks: {}",
        unexpected.join(", ")
    );
}

/// Guardrail: two fixers may claim the same declared file pattern only when
/// their required precedence is explicit in registry metadata.
#[test]
fn competing_fixers_must_not_share_declared_patterns() {
    let registry = builtin();
    let fixers: Vec<&Check> = registry
        .iter()
        .filter(|c| c.has_fix() && !c.patterns.is_empty())
        .collect();

    let mut conflicts = vec![];
    for (i, left) in fixers.iter().enumerate() {
        for right in fixers.iter().skip(i + 1) {
            if left.fix_after.contains(&right.name) || right.fix_after.contains(&left.name) {
                continue;
            }

            let overlap: Vec<&str> = left
                .patterns
                .iter()
                .copied()
                .filter(|p| right.patterns.contains(p))
                .collect();
            if !overlap.is_empty() {
                conflicts.push(format!(
                    "{} ({}) overlaps {} ({}) on {}",
                    left.name,
                    left.bin_name,
                    right.name,
                    right.bin_name,
                    overlap.join(", ")
                ));
            }
        }
    }

    assert!(
        conflicts.is_empty(),
        "competing fixer ownership detected:\n{}",
        conflicts.join("\n")
    );
}

/// Checks that every linter in the registry that uses an external binary
/// actually has that binary on PATH. Covers all registry entries, not just
/// those active in this repo — so tools like ktlint and hadolint are checked
/// even if they are not declared in this repo's mise.toml.
///
/// This test will fail on machines where not all linter tools are installed,
/// which is intentional: it identifies what is missing.
#[test]
fn all_registry_binaries_found() {
    let registry = builtin();

    let not_found: Vec<&str> = registry
        .iter()
        .filter(|c| c.uses_binary())
        .filter(|c| !binary_on_path(c.bin_name))
        .map(|c| c.name)
        .collect();

    assert!(
        not_found.is_empty(),
        "registry linters missing binary on PATH: {}",
        not_found.join(", ")
    );
}

#[test]
fn editorconfig_checker_json_is_optional_not_generated_baseline() {
    let registry = builtin();
    let check = registry
        .iter()
        .find(|check| check.name == "editorconfig-checker")
        .expect("editorconfig-checker exists");

    assert!(
        check.linter_config.is_some(),
        "existing .editorconfig-checker.json should still be passed to ec"
    );
    assert!(
        check.baseline_config.is_none(),
        ".editorconfig-checker.json should not be treated as generated baseline config"
    );
    assert!(
        check
            .baseline_triggers
            .iter()
            .any(|config| config.path == ".editorconfig"),
        ".editorconfig changes should trigger an all-files editorconfig-checker baseline"
    );
}

#[test]
fn default_renovate_preset_covers_all_linter_tools_weekly() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let default_json_path = manifest_dir.join("default.json");
    let default_json =
        std::fs::read_to_string(&default_json_path).expect("default.json must be readable");
    let parsed: serde_json::Value =
        serde_json::from_str(&default_json).expect("default.json must be valid JSON");

    let package_rules = parsed["packageRules"]
        .as_array()
        .expect("default.json packageRules must be an array");
    let linters_rule = package_rules
        .iter()
        .find(|rule| rule["groupName"].as_str() == Some("linters"))
        .expect("default.json must define a packageRules entry with groupName 'linters'");

    let actual = dep_names(linters_rule);
    let expected: Vec<&str> = builtin()
        .into_iter()
        .filter(|check| check.uses_binary())
        .filter(|check| !check.is_toolchain())
        .filter_map(|check| check.mise_tool_name.or(Some(check.bin_name)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    assert_eq!(
        actual, expected,
        "default.json weekly linters rule must stay sorted and in sync with the linter registry"
    );
    assert_eq!(
        actual,
        sorted_dep_names(linters_rule),
        "default.json weekly linters rule matchDepNames must be sorted"
    );

    assert_eq!(
        linters_rule["schedule"].as_array(),
        Some(&vec![serde_json::Value::String(
            "before 4am on Monday".to_string()
        )]),
        "linters package rule must remain on the weekly Monday schedule"
    );
    assert_eq!(
        linters_rule["commitMessageTopic"].as_str(),
        Some("flint-managed linter updates"),
        "linters package rule must keep the grouped PR title readable"
    );
    assert_eq!(
        linters_rule["separateMajorMinor"].as_bool(),
        Some(false),
        "linters package rule must keep major and non-major updates in one Monday PR"
    );
    assert!(
        !actual.contains(&"node"),
        "node is a runtime prerequisite, not a linter, and must not be in the weekly linters rule"
    );
}

#[test]
fn repo_renovate_config_stays_aligned_with_shared_preset_contract() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let default_json_path = manifest_dir.join("default.json");
    let repo_renovate_path = manifest_dir.join(".github/renovate.json5");

    let default_json =
        std::fs::read_to_string(&default_json_path).expect("default.json must be readable");
    let repo_renovate = std::fs::read_to_string(&repo_renovate_path)
        .expect(".github/renovate.json5 must be readable");

    let default_parsed: serde_json::Value =
        serde_json::from_str(&default_json).expect("default.json must be valid JSON");
    let repo_parsed: serde_json::Value =
        json5::from_str(&repo_renovate).expect(".github/renovate.json5 must be valid JSON5");

    for group_name in ["linters", "mise"] {
        let default_rule = package_rule_by_group_name(&default_parsed, group_name)
            .unwrap_or_else(|| panic!("default.json missing package rule {group_name:?}"));
        let repo_rule = package_rule_by_group_name(&repo_parsed, group_name).unwrap_or_else(|| {
            panic!(".github/renovate.json5 missing package rule {group_name:?}")
        });
        assert_eq!(
            default_rule["description"], repo_rule["description"],
            "package rule {group_name:?} description in .github/renovate.json5 drifted from default.json"
        );
        assert_eq!(
            default_rule["schedule"], repo_rule["schedule"],
            "package rule {group_name:?} schedule in .github/renovate.json5 drifted from default.json"
        );
        assert_eq!(
            default_rule["commitMessageTopic"], repo_rule["commitMessageTopic"],
            "package rule {group_name:?} commitMessageTopic in .github/renovate.json5 drifted from default.json"
        );
        assert_eq!(
            default_rule["separateMajorMinor"], repo_rule["separateMajorMinor"],
            "package rule {group_name:?} separateMajorMinor in .github/renovate.json5 drifted from default.json"
        );
        assert_eq!(
            rule_name_field(default_rule),
            rule_name_field(repo_rule),
            "package rule {group_name:?} matcher field in .github/renovate.json5 drifted from default.json"
        );
        assert_eq!(
            rule_names(default_rule),
            rule_names(repo_rule),
            "package rule {group_name:?} package matcher in .github/renovate.json5 drifted from default.json"
        );
        assert_eq!(
            rule_names(repo_rule),
            sorted_rule_names(repo_rule),
            "package rule {group_name:?} package matcher in .github/renovate.json5 must be sorted"
        );
    }

    assert_eq!(
        extract_version_rules(&default_parsed),
        extract_version_rules(&repo_parsed),
        "extractVersion overrides drifted between default.json and .github/renovate.json5"
    );
}

#[test]
fn linter_keys_include_mise_and_bare_tool_names() {
    let keys = linter_keys();
    assert!(keys.contains("aqua:owenlamont/ryl"));
    assert!(keys.contains("ryl"));
    assert!(keys.contains("aqua:jonwiggins/xmloxide"));
    assert!(keys.contains("xmllint"));
    assert!(keys.contains("aqua:grafana/flint"));
    assert!(keys.contains("github:grafana/flint"));
    assert!(keys.contains("cargo:https://github.com/grafana/flint"));
    assert!(keys.contains("cargo:https://github.com/grafana/flint.git"));
}

#[test]
fn flint_version_changed_detects_cargo_prerelease_rev_changes() {
    let previous = HashMap::from([(
        "cargo:https://github.com/grafana/flint".to_string(),
        "rev:aaaa".to_string(),
    )]);
    let current = HashMap::from([(
        "cargo:https://github.com/grafana/flint".to_string(),
        "rev:bbbb".to_string(),
    )]);

    assert!(flint_version_changed(&previous, &current));
}

#[test]
fn flint_version_changed_detects_release_to_cargo_backend_switch() {
    let previous = HashMap::from([("aqua:grafana/flint".to_string(), "0.20.4".to_string())]);
    let current = HashMap::from([(
        "cargo:https://github.com/grafana/flint".to_string(),
        "rev:bbbb".to_string(),
    )]);

    assert!(flint_version_changed(&previous, &current));
}

#[test]
fn runtime_version_changed_detects_node_updates_for_npm_checks() {
    let check = builtin()
        .into_iter()
        .find(|check| check.name == "renovate-deps")
        .expect("renovate-deps check");
    let previous = HashMap::from([
        ("node".to_string(), "22.0.0".to_string()),
        ("npm:renovate".to_string(), "43.136.3".to_string()),
    ]);
    let current = HashMap::from([
        ("node".to_string(), "24.0.0".to_string()),
        ("npm:renovate".to_string(), "43.136.3".to_string()),
    ]);

    assert!(runtime_version_changed(&check, &previous, &current));
}

#[test]
fn runtime_version_changed_detects_node_patch_updates_for_npm_checks() {
    let check = builtin()
        .into_iter()
        .find(|check| check.name == "renovate-deps")
        .expect("renovate-deps check");
    let previous = HashMap::from([
        ("node".to_string(), "24.0.0".to_string()),
        ("npm:renovate".to_string(), "43.136.3".to_string()),
    ]);
    let current = HashMap::from([
        ("node".to_string(), "24.0.1".to_string()),
        ("npm:renovate".to_string(), "43.136.3".to_string()),
    ]);

    assert!(runtime_version_changed(&check, &previous, &current));
}

#[test]
fn runtime_version_changed_ignores_node_updates_when_npm_tool_version_changed() {
    let check = builtin()
        .into_iter()
        .find(|check| check.name == "renovate-deps")
        .expect("renovate-deps check");
    let previous = HashMap::from([
        ("node".to_string(), "22.0.0".to_string()),
        ("npm:renovate".to_string(), "43.136.3".to_string()),
    ]);
    let current = HashMap::from([
        ("node".to_string(), "24.0.0".to_string()),
        ("npm:renovate".to_string(), "43.136.4".to_string()),
    ]);

    assert!(!runtime_version_changed(&check, &previous, &current));
}

#[test]
fn runtime_version_changed_ignores_node_updates_for_non_npm_checks() {
    let check = builtin()
        .into_iter()
        .find(|check| check.name == "shellcheck")
        .expect("shellcheck check");
    let previous = HashMap::from([
        ("node".to_string(), "22.0.0".to_string()),
        ("shellcheck".to_string(), "0.10.0".to_string()),
    ]);
    let current = HashMap::from([
        ("node".to_string(), "24.0.0".to_string()),
        ("shellcheck".to_string(), "0.10.0".to_string()),
    ]);

    assert!(!runtime_version_changed(&check, &previous, &current));
}

#[test]
fn full_baseline_runtime_changed_detects_node_updates_for_active_npm_tools() {
    let checks = builtin();
    let active: Vec<_> = checks
        .iter()
        .filter(|check| ["renovate-deps", "shellcheck"].contains(&check.name))
        .collect();
    let previous = HashMap::from([
        ("node".to_string(), "22.0.0".to_string()),
        ("npm:renovate".to_string(), "43.136.3".to_string()),
        ("shellcheck".to_string(), "0.10.0".to_string()),
    ]);
    let current = HashMap::from([
        ("node".to_string(), "24.0.0".to_string()),
        ("npm:renovate".to_string(), "43.136.3".to_string()),
        ("shellcheck".to_string(), "0.10.0".to_string()),
    ]);

    assert!(full_baseline_runtime_changed(&active, &previous, &current));
}
