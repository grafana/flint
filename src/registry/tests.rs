use std::collections::{BTreeSet, HashMap};
use std::path::Path;

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
            "replace with rumdl and yaml-lint, then remove prettier from the lint toolchain",
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

    let actual: BTreeSet<&str> = linters_rule["matchPackageNames"]
        .as_array()
        .expect("linters package rule must declare matchPackageNames")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("linters package names must be strings")
        })
        .collect();

    let mut expected: BTreeSet<&str> = builtin()
        .into_iter()
        .filter(|check| check.uses_binary())
        .filter(|check| !check.is_toolchain())
        .filter_map(|check| check.mise_tool_name.or(Some(check.bin_name)))
        .collect();
    // Backward-compatible alias still used in this repo's own mise.toml.
    expected.insert("github:koalaman/shellcheck");

    assert_eq!(
        linters_rule["schedule"].as_array(),
        Some(&vec![serde_json::Value::String(
            "before 4am on Monday".to_string()
        )]),
        "linters package rule must remain on the weekly Monday schedule"
    );
    assert!(
        !actual.contains("node"),
        "node is a runtime prerequisite, not a linter, and must not be in the weekly linters rule"
    );
    assert_eq!(
        actual, expected,
        "default.json weekly linters rule is out of sync with the linter registry"
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

fn package_names(rule: &serde_json::Value) -> BTreeSet<&str> {
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

    let scope = match &check.kind {
        CheckKind::Template { scope, .. } => match scope {
            Scope::File => "file",
            Scope::Files => "files",
            Scope::Project => "project",
        },
        CheckKind::Special(_) => "special",
    };
    rows.push(("Scope", format!("[{scope}](#scopes)")));

    if !check.patterns.is_empty() {
        rows.push(("Patterns", format!("`{}`", check.patterns.join(" "))));
    }

    match check.linter_config.as_ref() {
        Some(config) => rows.push(("Config", format!("`{}`", config.display_name()))),
        None => {
            if matches!(&check.kind, CheckKind::Special(SpecialKind::Links)) {
                rows.push(("Config", "via `[checks.links]` in flint.toml".to_string()));
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
