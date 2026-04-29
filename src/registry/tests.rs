use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use super::*;

#[path = "../readme_snippets.rs"]
mod readme_snippets;

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
fn shellcheck_alias_does_not_make_github_backend_obsolete() {
    let tools = HashMap::from([
        (
            "github:koalaman/shellcheck".to_string(),
            "0.11.0".to_string(),
        ),
        ("shellcheck".to_string(), "0.11.0".to_string()),
    ]);

    assert_eq!(find_obsolete_key(&tools), None);
}

#[test]
fn check_owned_tool_migrations_apply_after_v2_baseline() {
    let obsolete = obsolete_keys_after(crate::setup::V2_BASELINE_SETUP_VERSION);

    assert!(obsolete.contains(&("cargo:yaml-lint", "aqua:owenlamont/ryl")));
    assert!(obsolete.contains(&("github:owenlamont/ryl", "aqua:owenlamont/ryl")));
    assert!(obsolete.contains(&("pipx:ruff", "ruff")));
    assert!(obsolete.contains(&("github:astral-sh/ruff", "ruff")));
    assert!(obsolete.contains(&("shellcheck", "github:koalaman/shellcheck")));
    assert!(obsolete.contains(&("cargo:xmloxide", "github:jonwiggins/xmloxide")));
    assert!(obsolete_keys_after(crate::setup::LATEST_SUPPORTED_SETUP_VERSION).is_empty());
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
        crate::registry::CheckKind::Special(_) => return None,
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
        .filter(|check| !check.kind.is_special())
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
fn adaptive_checks_declare_relevance_hooks() {
    let missing: Vec<_> = builtin()
        .into_iter()
        .filter(|check| check.run_policy == RunPolicy::Adaptive)
        .filter(|check| check.adaptive_relevance.is_none())
        .map(|check| check.name)
        .collect();

    assert!(
        missing.is_empty(),
        "adaptive checks missing relevance hooks: {}",
        missing.join(", ")
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

    let actual = package_names(linters_rule);
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
        sorted_package_names(linters_rule),
        "default.json weekly linters rule matchPackageNames must be sorted"
    );

    assert_eq!(
        linters_rule["schedule"].as_array(),
        Some(&vec![serde_json::Value::String(
            "before 4am on Monday".to_string()
        )]),
        "linters package rule must remain on the weekly Monday schedule"
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
            package_names(default_rule),
            package_names(repo_rule),
            "package rule {group_name:?} matchPackageNames in .github/renovate.json5 drifted from default.json"
        );
        assert_eq!(
            package_names(repo_rule),
            sorted_package_names(repo_rule),
            "package rule {group_name:?} matchPackageNames in .github/renovate.json5 must be sorted"
        );
    }

    let description = "Update mise version in GitHub Actions workflows";
    let default_manager = custom_manager_by_description(&default_parsed, description)
        .unwrap_or_else(|| panic!("default.json missing custom manager {description:?}"));
    let repo_manager = custom_manager_by_description(&repo_parsed, description)
        .unwrap_or_else(|| panic!(".github/renovate.json5 missing custom manager {description:?}"));
    assert_eq!(
        default_manager, repo_manager,
        "custom manager {description:?} in .github/renovate.json5 drifted from default.json"
    );
}

#[test]
fn linter_keys_include_mise_and_bare_tool_names() {
    let keys = linter_keys();
    assert!(keys.contains("aqua:owenlamont/ryl"));
    assert!(keys.contains("ryl"));
    assert!(keys.contains("github:jonwiggins/xmloxide"));
    assert!(keys.contains("xmllint"));
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
    let previous = HashMap::from([("github:grafana/flint".to_string(), "0.20.4".to_string())]);
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

fn sorted_package_names(rule: &serde_json::Value) -> Vec<&str> {
    let mut names = package_names(rule);
    names.sort_unstable();
    names
}

fn extract_fenced_block_after<'a>(haystack: &'a str, marker: &str, lang: &str) -> &'a str {
    let start = haystack
        .find(marker)
        .unwrap_or_else(|| panic!("missing marker {marker:?}"));
    let after_marker = &haystack[start + marker.len()..];
    let fence = format!("```{lang}\n");
    let block_start = after_marker
        .find(&fence)
        .unwrap_or_else(|| panic!("missing {lang} fenced block after {marker:?}"))
        + fence.len();
    let rest = &after_marker[block_start..];
    let block_end = rest
        .find("\n```")
        .unwrap_or_else(|| panic!("missing closing fence after {marker:?}"));
    &rest[..block_end]
}

fn toml_tool_versions_from_table(
    table: &toml::Table,
    keys: &[&str],
) -> std::collections::BTreeMap<String, String> {
    keys.iter()
        .map(|key| {
            let value = table
                .get(*key)
                .unwrap_or_else(|| panic!("missing tool key {key:?}"));
            let version = value
                .as_str()
                .map(ToOwned::to_owned)
                .or_else(|| {
                    value
                        .as_table()
                        .and_then(|t| t.get("version"))
                        .and_then(toml::Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .unwrap_or_else(|| panic!("tool key {key:?} must have a string version"));
            ((*key).to_string(), version)
        })
        .collect()
}

#[test]
fn readme_quickstart_tools_snippets_stay_current() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme_path = manifest_dir.join("README.md");
    let mise_path = manifest_dir.join("mise.toml");

    let readme = std::fs::read_to_string(&readme_path).expect("README.md must be readable");
    let mise = std::fs::read_to_string(&mise_path).expect("mise.toml must be readable");

    let install_block =
        extract_fenced_block_after(&readme, readme_snippets::INSTALL_MARKER, "toml");
    let install_toml: toml::Value =
        toml::from_str(install_block).expect("README install block must be valid TOML");
    let install_tools = install_toml["tools"]
        .as_table()
        .expect("README install block must contain [tools]");
    assert_eq!(
        install_tools
            .get("github:grafana/flint")
            .and_then(toml::Value::as_str),
        Some(env!("CARGO_PKG_VERSION")),
        "README install snippet must pin the current flint release"
    );

    let quickstart_block =
        extract_fenced_block_after(&readme, readme_snippets::QUICKSTART_MARKER, "toml");
    let quickstart_toml: toml::Value =
        toml::from_str(quickstart_block).expect("README quickstart block must be valid TOML");
    let quickstart_tools = quickstart_toml["tools"]
        .as_table()
        .expect("README quickstart block must contain [tools]");

    let repo_mise: toml::Value = toml::from_str(&mise).expect("mise.toml must be valid TOML");
    let repo_tools = repo_mise["tools"]
        .as_table()
        .expect("repo mise.toml must contain [tools]");

    let expected = toml_tool_versions_from_table(repo_tools, readme_snippets::QUICKSTART_KEYS)
        .into_iter()
        .chain(std::iter::once((
            "github:grafana/flint".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        )))
        .collect::<std::collections::BTreeMap<_, _>>();

    let actual = toml_tool_versions_from_table(
        quickstart_tools,
        &[
            "github:grafana/flint",
            "github:koalaman/shellcheck",
            "shfmt",
            "actionlint",
            "rumdl",
            "ruff",
            "aqua:owenlamont/ryl",
            "taplo",
            "biome",
            "rust",
            "go",
            "lychee",
            "npm:renovate",
        ],
    );

    assert_eq!(
        actual, expected,
        "README quickstart [tools] snippet drifted from current repo tool versions"
    );
}

/// Verifies README summary table and docs/linters.md detail sections stay
/// in sync with the registry. The summary table lives in README.md between
/// `registry-table-*` markers; the per-linter detail sections live in
/// docs/linters.md between `linter-details-*` markers.
///
/// Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate.
#[test]
fn readme_linter_table_in_sync() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme_path = manifest_dir.join("README.md");
    let details_path = manifest_dir.join("docs/linters.md");
    let readme = std::fs::read_to_string(&readme_path).expect("README.md must be readable");
    let details = std::fs::read_to_string(&details_path).expect("docs/linters.md must be readable");
    let registry = builtin();

    let expected_summary = generate_summary_table(&registry);
    let expected_details = generate_linter_details(&registry);

    if std::env::var("UPDATE_README").is_ok() {
        let updated_readme = replace_section(
            &readme,
            README_TABLE_START,
            README_TABLE_END,
            &expected_summary,
        );
        let updated_details =
            replace_section(&details, DETAILS_START, DETAILS_END, &expected_details);
        std::fs::write(&readme_path, updated_readme).expect("failed to write README.md");
        std::fs::write(&details_path, updated_details).expect("failed to write docs/linters.md");
        return;
    }

    // Normalize both sides: strip blank lines that markdown formatters add around
    // headings, tables, and code blocks. This keeps the comparison stable
    // even when docs contain multi-paragraph content with blank lines.
    let actual_summary = extract_section(&readme, README_TABLE_START, README_TABLE_END);
    let actual_details = extract_section(&details, DETAILS_START, DETAILS_END);
    let expected_summary_norm = strip_blank_lines(&expected_summary);
    let expected_details_norm = strip_blank_lines(&expected_details);
    if actual_summary != expected_summary_norm {
        panic!(
            "README summary table is out of sync with the registry.\n\
             Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate.\n\n\
             Expected:\n{expected_summary_norm}\n\nActual:\n{actual_summary}"
        );
    }
    if actual_details != expected_details_norm {
        panic!(
            "docs/linters.md detail sections out of sync with the registry.\n\
             Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate.\n\n\
             Expected:\n{expected_details_norm}\n\nActual:\n{actual_details}"
        );
    }
}

const README_TABLE_START: &str = "<!-- registry-table-start -->";
const README_TABLE_END: &str = "<!-- registry-table-end -->";
const DETAILS_START: &str = "<!-- linter-details-start -->";
const DETAILS_END: &str = "<!-- linter-details-end -->";
const GENERATED_COMMENT: &str = "<!-- Generated. Run `UPDATE_README=1 cargo test readme_linter_table_in_sync` to regenerate. -->";

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

fn generate_summary_table(registry: &[Check]) -> String {
    // Summary table: Name | Description | Fix — sorted alphabetically.
    // Name column links to the matching detail section in docs/linters.md.
    let headers = ["Name", "Description", "Fix"];
    let mut sorted: Vec<&Check> = registry.iter().collect();
    sorted.sort_by_key(|c| c.name);
    let rows: Vec<[String; 3]> = sorted.iter().map(|c| summary_row(c)).collect();

    let mut widths = headers.map(|h| h.len());
    for row in &rows {
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

    let mut lines = vec![
        GENERATED_COMMENT.to_string(),
        fmt_row(&header_strs),
        sep_row,
    ];
    for row in &rows {
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
        lines.push(format!("## `{}`", check.name));
        lines.push(detail_table(check));
    }
    lines.join("\n")
}

fn summary_row(check: &Check) -> [String; 3] {
    // docs/linters.md uses `## `<name>`` — GitHub strips backticks and
    // lowercases to produce the anchor `<name>`.
    let name = format!("[`{0}`](docs/linters.md#{0})", check.name);
    let desc = if check.desc.is_empty() {
        "—".to_string()
    } else {
        check.desc.to_string()
    };
    let fix = if check.has_fix() { "yes" } else { "—" }.to_string();
    [name, desc, fix]
}

fn detail_table(check: &Check) -> String {
    let rows = detail_rows(check);

    let col1_w = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let col2_w = rows.iter().map(|(_, v)| v.len()).max().unwrap_or(0);

    let fmt = |k: &str, v: &str| format!("| {:<col1_w$} | {:<col2_w$} |", k, v);
    let sep = format!("| {} | {} |", "-".repeat(col1_w), "-".repeat(col2_w));

    // Empty header row: markdown requires one, but we don't need visible
    // column labels — Description and Fix are data rows, not headers.
    let mut lines = vec![fmt("", ""), sep];
    for (k, v) in &rows {
        lines.push(fmt(k, v));
    }
    if !check.docs.is_empty() {
        lines.push(check.docs.to_string());
    }
    lines.join("\n")
}

fn detail_rows(check: &Check) -> Vec<(&'static str, String)> {
    let mut rows: Vec<(&'static str, String)> = vec![];

    if !check.desc.is_empty() {
        rows.push(("Description", check.desc.to_string()));
    }

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

    match check.linter_config.as_ref() {
        Some(config) => rows.push(("Config", format!("`{}`", config.display_name()))),
        None => {
            if let Some(config) = check.kind.special_config_display() {
                rows.push(("Config", config.to_string()));
            }
        }
    }

    match check.run_policy {
        crate::registry::RunPolicy::Fast => {}
        crate::registry::RunPolicy::Slow => {
            rows.push(("Run policy", "slow — skipped by `--fast-only`".to_string()));
        }
        crate::registry::RunPolicy::Adaptive => {
            rows.push((
                "Run policy",
                "adaptive — runs in `--fast-only` only when relevant".to_string(),
            ));
        }
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
