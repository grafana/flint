use std::collections::HashMap;

use super::super::*;

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
fn registry_entries_have_complete_metadata() {
    for check in builtin() {
        assert!(
            !check.desc.is_empty(),
            "{} is missing a description",
            check.name
        );

        if check.uses_binary() {
            assert!(
                check.install_key().is_some() || check.activate_unconditionally,
                "{} uses a binary but has no install key",
                check.name
            );
            assert!(
                check.project_url.is_some(),
                "{} uses a binary but has no upstream project URL",
                check.name
            );
        }

        if check.linter_config.is_some() {
            assert!(
                check.config_doc_url.is_some(),
                "{} has a config file but no config documentation URL",
                check.name
            );
        }
    }
}

#[test]
fn regex_replace_fixes_before_all_other_checks() {
    let check = builtin()
        .into_iter()
        .find(|check| check.name == "regex-replace")
        .unwrap();

    assert!(check.fix_first);
}

#[test]
fn fixer_dependencies_are_valid_and_acyclic() {
    let registry = builtin();
    let fixers: Vec<&Check> = registry.iter().filter(|check| check.has_fix()).collect();

    for check in &fixers {
        for dependency in &check.fix_after {
            assert_ne!(check.name, *dependency, "{} depends on itself", check.name);
            assert!(
                fixers.iter().any(|candidate| candidate.name == *dependency),
                "{} must run after unknown or non-fix-capable check {}",
                check.name,
                dependency
            );
        }
    }

    let mut remaining = fixers;
    while !remaining.is_empty() {
        let next = remaining.iter().position(|check| {
            check.fix_after.iter().all(|dependency| {
                !remaining
                    .iter()
                    .any(|candidate| candidate.name == *dependency)
            })
        });
        assert!(
            next.is_some(),
            "cyclic fixer ordering involving: {}",
            remaining
                .iter()
                .map(|check| check.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        remaining.remove(next.unwrap());
    }
}

#[test]
fn rumdl_batches_matching_files() {
    let check = builtin()
        .into_iter()
        .find(|check| check.name == "rumdl")
        .unwrap();

    assert!(matches!(
        check.kind,
        CheckKind::Template {
            check_cmd: "rumdl check {FILES}",
            fix_cmd: "rumdl check --fix {FILES}",
            scope: Scope::Files,
            ..
        }
    ));
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
