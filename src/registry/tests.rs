use std::collections::HashMap;
use std::path::Path;

use super::*;

#[test]
fn find_obsolete_key_detects_superseded_keys() {
    let mut tools = HashMap::new();
    tools.insert("npm:markdownlint-cli".to_string(), "0.39.0".to_string());
    let result = find_obsolete_key(&tools);
    assert_eq!(
        result,
        Some(("npm:markdownlint-cli", "npm:markdownlint-cli2"))
    );
}

#[test]
fn find_obsolete_key_returns_none_for_clean_tools() {
    let mut tools = HashMap::new();
    tools.insert("npm:markdownlint-cli2".to_string(), "0.17.2".to_string());
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

    // Normalize both sides: strip blank lines that prettier adds around
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

    match check.linter_config {
        Some((filename, _)) => rows.push(("Config", format!("`{filename}`"))),
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
