use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use regex::Regex;

use super::*;

#[test]
fn find_obsolete_key_returns_none_for_clean_tools() {
    let mut tools = HashMap::new();
    tools.insert("shfmt".to_string(), "3.13.1".to_string());
    assert_eq!(find_obsolete_key(&tools), None);
}

#[test]
fn find_obsolete_key_detects_legacy_shfmt_backend() {
    let mut tools = HashMap::new();
    tools.insert("github:mvdan/sh".to_string(), "v3.12.0".to_string());
    assert_eq!(
        find_obsolete_key(&tools),
        Some(("github:mvdan/sh", "shfmt"))
    );
}

#[test]
fn find_obsolete_key_detects_legacy_biome_backend() {
    let mut tools = HashMap::new();
    tools.insert("npm:@biomejs/biome".to_string(), "2.4.12".to_string());
    assert_eq!(
        find_obsolete_key(&tools),
        Some(("npm:@biomejs/biome", "biome"))
    );
}

#[test]
fn find_obsolete_key_detects_legacy_yaml_lint_backend() {
    let mut tools = HashMap::new();
    tools.insert("cargo:yaml-lint".to_string(), "0.1.0".to_string());
    assert_eq!(
        find_obsolete_key(&tools),
        Some(("cargo:yaml-lint", "aqua:owenlamont/ryl"))
    );
}

#[test]
fn find_obsolete_key_detects_legacy_ruff_backend() {
    let mut tools = HashMap::new();
    tools.insert("pipx:ruff".to_string(), "0.15.0".to_string());
    assert_eq!(find_obsolete_key(&tools), Some(("pipx:ruff", "ruff")));
}

#[test]
fn shellcheck_github_backend_is_obsolete_even_when_bare_key_exists() {
    let tools = HashMap::from([
        (
            "github:koalaman/shellcheck".to_string(),
            "0.11.0".to_string(),
        ),
        ("shellcheck".to_string(), "0.11.0".to_string()),
    ]);

    assert_eq!(
        find_obsolete_key(&tools),
        Some(("github:koalaman/shellcheck", "shellcheck"))
    );
}

#[test]
fn check_owned_tool_migrations_are_always_actionable() {
    let obsolete = obsolete_keys();

    assert!(obsolete.contains(&("cargo:yaml-lint", "aqua:owenlamont/ryl")));
    assert!(obsolete.contains(&("github:owenlamont/ryl", "aqua:owenlamont/ryl")));
    assert!(obsolete.contains(&("pipx:ruff", "ruff")));
    assert!(obsolete.contains(&("github:astral-sh/ruff", "ruff")));
    assert!(obsolete.contains(&("github:koalaman/shellcheck", "shellcheck")));
    assert!(obsolete.contains(&("cargo:xmloxide", "aqua:jonwiggins/xmloxide")));
}

#[test]
fn registry_tool_key_migrations_are_unique_and_have_targets() {
    let mut seen = std::collections::HashSet::new();

    for check in builtin() {
        if check.tool_key_migrations.is_empty() {
            continue;
        }
        assert!(
            check.install_key().is_some(),
            "{} declares tool-key migrations but has no install key",
            check.name
        );
        for migration in &check.tool_key_migrations {
            assert!(
                seen.insert(migration.old_key),
                "duplicate registry tool-key migration: {}",
                migration.old_key
            );
        }
    }
}

#[test]
fn find_unsupported_key_detects_markdownlint_stack() {
    let mut tools = HashMap::new();
    tools.insert("npm:markdownlint-cli2".to_string(), "0.18.1".to_string());
    assert_eq!(
        find_unsupported_key(&tools),
        Some((
            "npm:markdownlint-cli2",
            "replace with rumdl and remove markdownlint-era config",
        ))
    );
}

#[test]
fn find_unsupported_key_detects_legacy_markdownlint_cli_stack() {
    let mut tools = HashMap::new();
    tools.insert("npm:markdownlint-cli".to_string(), "0.39.0".to_string());
    assert_eq!(
        find_unsupported_key(&tools),
        Some((
            "npm:markdownlint-cli",
            "replace with rumdl and remove markdownlint-era config",
        ))
    );
}

#[test]
fn find_unsupported_key_detects_prettier_stack() {
    let mut tools = HashMap::new();
    tools.insert("npm:prettier".to_string(), "3.6.2".to_string());
    assert_eq!(
        find_unsupported_key(&tools),
        Some((
            "npm:prettier",
            "replace with rumdl and ryl, then remove prettier from the lint toolchain",
        ))
    );
}

/// If any entry for a bin_name declares a version_range, every entry for that
/// bin_name must declare one. A mix of ranged and unranged entries for the same
/// binary is ambiguous — it would be impossible to guarantee exactly one activates.
/// (Multiple unranged entries for the same binary are fine: they're different
/// subcommand invocations of the same tool, e.g. `biome check` vs `biome format`.)
#[test]
fn version_ranges_must_not_be_mixed_with_unranged_entries() {
    let registry = builtin();
    let mut by_bin: HashMap<&str, Vec<&Check>> = HashMap::new();
    for check in &registry {
        by_bin.entry(check.bin_name).or_default().push(check);
    }
    for (bin, checks) in &by_bin {
        let any_ranged = checks.iter().any(|c| c.version_range.is_some());
        if any_ranged {
            for check in checks {
                assert!(
                    check.version_range.is_some(),
                    "check '{}' shares bin_name '{}' with version-ranged entries but has no version_range",
                    check.name,
                    bin,
                );
            }
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
fn test_case_groups_match_registered_checks() {
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

/// Guardrail: two different fixer tools should not claim the same declared file
/// pattern. Overlap between checks from the same underlying tool is still
/// allowed for now (e.g. `biome` + `biome-format`, `ruff` + `ruff-format`)
/// because those pairs are intentionally split into lint and format modes.
#[test]
fn competing_fixers_must_not_share_declared_patterns() {
    const ALLOWED_OVERLAPS: &[(&str, &str)] = &[
        // markdownlint enforces rules; prettier canonicalizes formatting.
        ("markdownlint-cli2", "prettier"),
        // clippy and rustfmt both fix Rust files, but serve distinct purposes.
        ("cargo-clippy", "cargo-fmt"),
    ];

    let registry = builtin();
    let fixers: Vec<&Check> = registry
        .iter()
        .filter(|c| c.has_fix() && !c.patterns.is_empty())
        .collect();

    let mut conflicts = vec![];
    for (i, left) in fixers.iter().enumerate() {
        for right in fixers.iter().skip(i + 1) {
            if left.bin_name == right.bin_name {
                continue;
            }
            let pair = if left.name < right.name {
                (left.name, right.name)
            } else {
                (right.name, left.name)
            };
            if ALLOWED_OVERLAPS.contains(&pair) {
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

    {
        let description = "Update mise version in GitHub Actions workflows";
        let default_manager = custom_manager_by_description(&default_parsed, description)
            .unwrap_or_else(|| panic!("default.json missing custom manager {description:?}"));
        let repo_manager =
            custom_manager_by_description(&repo_parsed, description).unwrap_or_else(|| {
                panic!(".github/renovate.json5 missing custom manager {description:?}")
            });
        assert_eq!(
            default_manager, repo_manager,
            "custom manager {description:?} in .github/renovate.json5 drifted from default.json"
        );
    }
}

#[test]
fn mise_action_custom_manager_matches_plain_and_block_scalar_sha256() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let default_json_path = manifest_dir.join("default.json");
    let default_json =
        std::fs::read_to_string(&default_json_path).expect("default.json must be readable");
    let parsed: serde_json::Value =
        serde_json::from_str(&default_json).expect("default.json must be valid JSON");

    let description = "Update mise version in GitHub Actions workflows";
    let manager = custom_manager_by_description(&parsed, description)
        .unwrap_or_else(|| panic!("default.json missing custom manager {description:?}"));
    let pattern = manager["matchStrings"][0]
        .as_str()
        .expect("custom manager match string must be a string");
    let regex = Regex::new(pattern).expect("custom manager regex must compile");

    for (name, sample) in [
        (
            "plain scalar",
            r#"
      - name: Setup mise
        uses: jdx/mise-action@1648a7812b9aeae629881980618f079932869151 # v4.0.1
        with:
          version: v2026.4.28
          sha256: 9655492db554e8f70a69830f54307ac0f4681d6c42f9844e862528b7853d09d1
"#,
        ),
        (
            "block scalar",
            r#"
      - name: Setup mise
        uses: jdx/mise-action@1648a7812b9aeae629881980618f079932869151 # v4.0.1
        with:
          version: v2026.4.28
          sha256: >-
            c55befc52e5694f388b927ef304362ca7b9e919d97d43c342fca57f2eccea255
"#,
        ),
    ] {
        let captures = regex
            .captures(sample)
            .unwrap_or_else(|| panic!("regex must match {name} mise-action YAML"));
        assert_eq!(
            captures.name("currentValue").map(|value| value.as_str()),
            Some("v2026.4.28"),
            "regex must capture the mise version for {name}"
        );
        assert_eq!(
            captures.name("currentDigest").map(|value| value.as_str()),
            Some(match name {
                "plain scalar" =>
                    "9655492db554e8f70a69830f54307ac0f4681d6c42f9844e862528b7853d09d1",
                "block scalar" =>
                    "c55befc52e5694f388b927ef304362ca7b9e919d97d43c342fca57f2eccea255",
                _ => unreachable!("unexpected sample"),
            }),
            "regex must capture the mise sha256 digest for {name}"
        );
    }
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

fn package_rule_by_group_name<'a>(
    parsed: &'a serde_json::Value,
    group_name: &str,
) -> Option<&'a serde_json::Value> {
    parsed["packageRules"]
        .as_array()?
        .iter()
        .find(|rule| rule["groupName"].as_str() == Some(group_name))
}

fn custom_manager_by_description<'a>(
    parsed: &'a serde_json::Value,
    description: &str,
) -> Option<&'a serde_json::Value> {
    parsed["customManagers"]
        .as_array()?
        .iter()
        .find(|manager| manager["description"].as_str() == Some(description))
}

fn package_names(rule: &serde_json::Value) -> Vec<&str> {
    rule["matchPackageNames"]
        .as_array()
        .expect("package rule must declare matchPackageNames")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("package rule matchPackageNames entries must be strings")
        })
        .collect()
}

fn dep_names(rule: &serde_json::Value) -> Vec<&str> {
    rule["matchDepNames"]
        .as_array()
        .expect("package rule must declare matchDepNames")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("package rule matchDepNames entries must be strings")
        })
        .collect()
}

fn sorted_dep_names(rule: &serde_json::Value) -> Vec<&str> {
    let mut names = dep_names(rule);
    names.sort_unstable();
    names
}

fn rule_name_field(rule: &serde_json::Value) -> &'static str {
    match (
        rule.get("matchDepNames").is_some(),
        rule.get("matchPackageNames").is_some(),
    ) {
        (true, false) => "matchDepNames",
        (false, true) => "matchPackageNames",
        (true, true) => {
            panic!("package rule must not declare both matchDepNames and matchPackageNames")
        }
        (false, false) => {
            panic!("package rule must declare matchDepNames or matchPackageNames")
        }
    }
}

fn rule_names(rule: &serde_json::Value) -> Vec<&str> {
    match rule_name_field(rule) {
        "matchDepNames" => dep_names(rule),
        "matchPackageNames" => package_names(rule),
        _ => unreachable!("unexpected rule_name_field result"),
    }
}

fn sorted_rule_names(rule: &serde_json::Value) -> Vec<&str> {
    let mut names = rule_names(rule);
    names.sort_unstable();
    names
}

/// Verifies README summary table and docs/linters.md detail sections stay
/// in sync with the registry. The summary table lives in README.md between
/// `registry-table-*` markers; the same overview tables live in docs/linters.md
/// between `linter-overview-*` markers; the per-linter detail sections live in
/// docs/linters.md between `linter-details-*` markers.
///
/// Run `mise run generate` to regenerate.
#[test]
fn readme_linter_table_in_sync() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme_path = manifest_dir.join("README.md");
    let details_path = manifest_dir.join("docs/linters.md");
    let readme = std::fs::read_to_string(&readme_path).expect("README.md must be readable");
    let details = std::fs::read_to_string(&details_path).expect("docs/linters.md must be readable");
    let registry = builtin();

    let expected_summary = generate_overview_tables(&registry, OverviewLinkTarget::Readme);
    let expected_overview = generate_overview_tables(&registry, OverviewLinkTarget::LinterPage);
    let expected_details = generate_linter_details(&registry);

    if std::env::var("UPDATE_README").is_ok() {
        let updated_readme = replace_section(
            &readme,
            README_TABLE_START,
            README_TABLE_END,
            &expected_summary,
        );
        let updated_details = replace_section(
            &replace_section(&details, OVERVIEW_START, OVERVIEW_END, &expected_overview),
            DETAILS_START,
            DETAILS_END,
            &expected_details,
        );
        std::fs::write(&readme_path, updated_readme).expect("failed to write README.md");
        std::fs::write(&details_path, updated_details).expect("failed to write docs/linters.md");
        return;
    }

    // Normalize both sides: strip blank lines that markdown formatters add around
    // headings, tables, and code blocks. This keeps the comparison stable
    // even when docs contain multi-paragraph content with blank lines.
    let actual_summary = extract_section(&readme, README_TABLE_START, README_TABLE_END);
    let actual_overview = extract_section(&details, OVERVIEW_START, OVERVIEW_END);
    let actual_details = extract_section(&details, DETAILS_START, DETAILS_END);
    let expected_summary_norm = strip_blank_lines(&expected_summary);
    let expected_overview_norm = strip_blank_lines(&expected_overview);
    let expected_details_norm = strip_blank_lines(&expected_details);
    if actual_summary != expected_summary_norm {
        panic!(
            "README summary table is out of sync with the registry.\n\
             Run `mise run generate` to regenerate.\n\n\
             Expected:\n{expected_summary_norm}\n\nActual:\n{actual_summary}"
        );
    }
    if actual_overview != expected_overview_norm {
        panic!(
            "docs/linters.md overview tables are out of sync with the registry.\n\
             Run `mise run generate` to regenerate.\n\n\
             Expected:\n{expected_overview_norm}\n\nActual:\n{actual_overview}"
        );
    }
    if actual_details != expected_details_norm {
        panic!(
            "docs/linters.md detail sections out of sync with the registry.\n\
             Run `mise run generate` to regenerate.\n\n\
             Expected:\n{expected_details_norm}\n\nActual:\n{actual_details}"
        );
    }
}

const README_TABLE_START: &str = "<!-- registry-table-start -->";
const README_TABLE_END: &str = "<!-- registry-table-end -->";
const OVERVIEW_START: &str = "<!-- linter-overview-start -->";
const OVERVIEW_END: &str = "<!-- linter-overview-end -->";
const DETAILS_START: &str = "<!-- linter-details-start -->";
const DETAILS_END: &str = "<!-- linter-details-end -->";
const GENERATED_COMMENT: &str = "<!-- Generated. Run `mise run generate` to regenerate. -->";

fn strip_blank_lines(s: &str) -> String {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_section(haystack: &str, start_marker: &str, end_marker: &str) -> String {
    let start = haystack
        .find(start_marker)
        .unwrap_or_else(|| panic!("missing {start_marker} marker"))
        + start_marker.len();
    let end = haystack
        .find(end_marker)
        .unwrap_or_else(|| panic!("missing {end_marker} marker"));
    strip_blank_lines(&haystack[start..end])
}

fn replace_section(haystack: &str, start_marker: &str, end_marker: &str, body: &str) -> String {
    let start = haystack
        .find(start_marker)
        .unwrap_or_else(|| panic!("missing {start_marker} marker"))
        + start_marker.len();
    let end = haystack
        .find(end_marker)
        .unwrap_or_else(|| panic!("missing {end_marker} marker"));
    format!(
        "{}\n{}\n{}{}",
        &haystack[..start],
        body,
        end_marker,
        &haystack[end + end_marker.len()..]
    )
}

#[derive(Clone, Copy)]
enum OverviewLinkTarget {
    Readme,
    LinterPage,
}

impl OverviewLinkTarget {
    fn heading_prefix(self) -> &'static str {
        match self {
            Self::Readme => "###",
            Self::LinterPage => "###",
        }
    }
}

#[derive(Default)]
struct OverviewDocRow {
    linter: Option<String>,
    formatter: Option<String>,
    checks: Vec<String>,
    description: Option<&'static str>,
}

fn generate_overview_tables(registry: &[Check], link_target: OverviewLinkTarget) -> String {
    use crate::registry::types::{OverviewRole, OverviewSection};
    use std::collections::BTreeMap;

    let mut sections: BTreeMap<OverviewSection, BTreeMap<&'static str, OverviewDocRow>> =
        BTreeMap::new();

    for check in registry {
        for overview in &check.overviews {
            let row = sections
                .entry(overview.section)
                .or_default()
                .entry(overview.row_name)
                .or_default();
            let link = overview_name_cell(check, link_target);
            match overview.role {
                OverviewRole::Linter => row.linter = Some(link),
                OverviewRole::Formatter => row.formatter = Some(link),
                OverviewRole::Check => row.checks.push(link),
                OverviewRole::Both => {
                    row.linter = Some(link.clone());
                    row.formatter = Some(link);
                }
            }
            if let Some(description) = overview.description {
                row.description = Some(description);
            }
        }
    }

    let lines = vec![
        GENERATED_COMMENT.to_string(),
        format!(
            "{} {}",
            link_target.heading_prefix(),
            OverviewSection::Languages.title()
        ),
        render_markdown_table(
            &["Name", "Linter", "Formatter"],
            &render_overview_rows(&sections, OverviewSection::Languages),
        ),
        format!(
            "{} {}",
            link_target.heading_prefix(),
            OverviewSection::FilesFormats.title()
        ),
        render_markdown_table(
            &["Name", "Linter", "Formatter"],
            &render_overview_rows(&sections, OverviewSection::FilesFormats),
        ),
        format!(
            "{} {}",
            link_target.heading_prefix(),
            OverviewSection::ToolingCi.title()
        ),
        render_markdown_table(
            &["Name", "Check"],
            &render_check_rows(&sections, OverviewSection::ToolingCi),
        ),
        format!(
            "{} {}",
            link_target.heading_prefix(),
            OverviewSection::General.title()
        ),
        render_markdown_table(
            &["Name", "Check", "Description"],
            &render_general_rows(&sections),
        ),
    ];
    lines.join("\n\n")
}

fn render_overview_rows(
    sections: &std::collections::BTreeMap<
        crate::registry::types::OverviewSection,
        std::collections::BTreeMap<&'static str, OverviewDocRow>,
    >,
    section: crate::registry::types::OverviewSection,
) -> Vec<[String; 3]> {
    sections
        .get(&section)
        .into_iter()
        .flat_map(|rows| rows.iter())
        .map(|(name, row)| {
            [
                (*name).to_string(),
                row.linter.clone().unwrap_or_else(|| "—".to_string()),
                row.formatter.clone().unwrap_or_else(|| "—".to_string()),
            ]
        })
        .collect()
}

fn render_check_rows(
    sections: &std::collections::BTreeMap<
        crate::registry::types::OverviewSection,
        std::collections::BTreeMap<&'static str, OverviewDocRow>,
    >,
    section: crate::registry::types::OverviewSection,
) -> Vec<[String; 2]> {
    sections
        .get(&section)
        .into_iter()
        .flat_map(|rows| rows.iter())
        .map(|(name, row)| {
            [
                (*name).to_string(),
                if row.checks.is_empty() {
                    "—".to_string()
                } else {
                    row.checks.join(" / ")
                },
            ]
        })
        .collect()
}

fn render_general_rows(
    sections: &std::collections::BTreeMap<
        crate::registry::types::OverviewSection,
        std::collections::BTreeMap<&'static str, OverviewDocRow>,
    >,
) -> Vec<[String; 3]> {
    sections
        .get(&crate::registry::types::OverviewSection::General)
        .into_iter()
        .flat_map(|rows| rows.iter())
        .map(|(name, row)| {
            [
                (*name).to_string(),
                if row.checks.is_empty() {
                    "—".to_string()
                } else {
                    row.checks.join(" / ")
                },
                row.description.unwrap_or("—").to_string(),
            ]
        })
        .collect()
}

fn render_markdown_table<const N: usize>(headers: &[&str; N], rows: &[[String; N]]) -> String {
    let mut widths = headers.map(|h| h.len());
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }
    let fmt_row = |cells: &[&str]| -> String {
        let cols: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(i, cell)| format!("{:<width$}", cell, width = widths[i]))
            .collect();
        format!("| {} |", cols.join(" | "))
    };
    let separator: Vec<String> = widths.iter().map(|&w| "-".repeat(w)).collect();
    let sep_row = format!("| {} |", separator.join(" | "));
    let header_strs: Vec<&str> = headers.to_vec();

    let mut lines = vec![fmt_row(&header_strs), sep_row];
    for row in rows {
        let strs: Vec<&str> = row.iter().map(|s| s.as_str()).collect();
        lines.push(fmt_row(&strs));
    }
    lines.join("\n")
}

fn generate_linter_details(registry: &[Check]) -> String {
    let mut sorted: Vec<&Check> = registry.iter().collect();
    sorted.sort_by_key(|c| c.name);

    let mut lines = vec![GENERATED_COMMENT.to_string()];
    for check in &sorted {
        let heading = match check.project_url {
            Some(url) => format!("### [`{}`]({url})", check.name),
            None => format!("### `{}`", check.name),
        };
        lines.push(heading);
        lines.push(detail_table(check));
    }
    lines.join("\n")
}

fn overview_name_cell(check: &Check, link_target: OverviewLinkTarget) -> String {
    match link_target {
        OverviewLinkTarget::Readme => format!("[`{}`]({})", check.name, detail_link(check)),
        OverviewLinkTarget::LinterPage => format!("[`{}`](#{})", check.name, check.name),
    }
}

fn detail_link(check: &Check) -> String {
    // docs/linters.md uses `## `<name>`` — GitHub strips backticks and
    // lowercases to produce the anchor `<name>`.
    format!("docs/linters.md#{}", check.name)
}

fn detail_table(check: &Check) -> String {
    let rows = detail_rows(check);

    let col1_w = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let col2_w = rows.iter().map(|(_, v)| v.len()).max().unwrap_or(0);

    let fmt = |k: &str, v: &str| format!("| {:<col1_w$} | {:<col2_w$} |", k, v);
    let sep = format!("| {} | {} |", "-".repeat(col1_w), "-".repeat(col2_w));

    // Empty header row: markdown requires one, but we don't need visible
    // column labels for the metadata table.
    let mut lines = vec![fmt("", ""), sep];
    for (k, v) in &rows {
        lines.push(fmt(k, v));
    }
    if !check.desc.is_empty() {
        lines.push(String::new());
        lines.push(check.desc.to_string());
    }
    if !check.docs.is_empty() {
        lines.push(String::new());
        lines.push(check.docs.to_string());
    }
    lines.join("\n")
}

fn detail_rows(check: &Check) -> Vec<(&'static str, String)> {
    let mut rows: Vec<(&'static str, String)> = vec![];

    rows.push((
        "Fix",
        if check.has_fix() { "yes" } else { "no" }.to_string(),
    ));

    let binary = if check.uses_binary() {
        format!("`{}`", check.bin_name)
    } else {
        "(built-in)".to_string()
    };
    rows.push(("Binary", binary));

    let scope = check.kind.scope_name();
    rows.push(("Scope", format!("[{scope}](#scope-{scope})")));

    if !check.patterns.is_empty() {
        rows.push(("Patterns", format!("`{}`", check.patterns.join(" "))));
    }

    match (
        check.linter_config.as_ref(),
        check.baseline_config.as_ref(),
        check.kind.native_config_display(),
    ) {
        (Some(config), _, _) => {
            let value = match check.config_doc_url {
                Some(url) => format!("[`{}`]({url})", config.display_name()),
                None => format!("`{}`", config.display_name()),
            };
            rows.push(("Config", value));
        }
        (None, Some(config), _) => {
            let value = match check.config_doc_url {
                Some(url) => format!("[`{}`]({url})", config.path),
                None => format!("`{}`", config.path),
            };
            rows.push(("Config", value));
        }
        (None, None, Some(config)) => rows.push(("Config", config.to_string())),
        (None, None, None) => {}
    }

    if check.adaptive_relevance.is_some() {
        let label = if check.name == "renovate-deps" {
            "adaptive — see [when does this run?](linters/renovate-deps.md#when-does-this-run)"
                .to_string()
        } else {
            "adaptive — runs on local default runs only when changed files are relevant".to_string()
        };
        rows.push(("Run policy", label));
    }

    rows
}
/// Smoke test: every check whose tool key resolves in this repo's expanded
/// mise_tools map must pass check_active. This catches tool-name mismatches
/// (wrong lookup key) and version-range violations without a hardcoded list —
/// new registry entries are covered automatically.
#[test]
fn all_flint_repo_linters_detected() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mise_tools = read_mise_tools(project_root);
    let registry = builtin();

    let inactive: Vec<&str> = registry
        .iter()
        .filter(|c| {
            // A check is "expected" if its lookup key appears in the expanded
            // mise_tools map, or if it activates unconditionally.
            c.activate_unconditionally || {
                let lookup = c.mise_tool_name.unwrap_or(c.bin_name);
                mise_tools.contains_key(lookup)
            }
        })
        .filter(|c| !check_active(c, &mise_tools))
        .map(|c| c.name)
        .collect();

    assert!(
        inactive.is_empty(),
        "linters not detected in flint repo: {}",
        inactive.join(", ")
    );
}
