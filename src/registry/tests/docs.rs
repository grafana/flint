use std::collections::BTreeSet;
use std::path::Path;

use super::super::*;

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

    if std::env::var("UPDATE_README").is_ok() {
        let updated_readme = replace_section(
            &readme,
            README_TABLE_START,
            README_TABLE_END,
            &expected_summary,
        );
        let updated_details =
            replace_section(&details, OVERVIEW_START, OVERVIEW_END, &expected_overview);
        std::fs::write(&readme_path, updated_readme).expect("failed to write README.md");
        std::fs::write(&details_path, updated_details).expect("failed to write docs/linters.md");
        update_linter_pages(manifest_dir, &registry);
        verify_linter_pages(manifest_dir, &registry);
        return;
    }

    // Normalize both sides: strip blank lines that markdown formatters add around
    // headings, tables, and code blocks. This keeps the comparison stable
    // even when docs contain multi-paragraph content with blank lines.
    let actual_summary = extract_section(&readme, README_TABLE_START, README_TABLE_END);
    let actual_overview = extract_section(&details, OVERVIEW_START, OVERVIEW_END);
    let expected_summary_norm = strip_blank_lines(&expected_summary);
    let expected_overview_norm = strip_blank_lines(&expected_overview);
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
    verify_linter_pages(manifest_dir, &registry);
}

const README_TABLE_START: &str = "<!-- registry-table-start -->";
const README_TABLE_END: &str = "<!-- registry-table-end -->";
const OVERVIEW_START: &str = "<!-- linter-overview-start -->";
const OVERVIEW_END: &str = "<!-- linter-overview-end -->";
const METADATA_START: &str = "<!-- linter-metadata-start -->";
const METADATA_END: &str = "<!-- linter-metadata-end -->";
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

fn overview_name_cell(check: &Check, link_target: OverviewLinkTarget) -> String {
    match link_target {
        OverviewLinkTarget::Readme => format!("[`{}`]({})", check.name, detail_link(check)),
        OverviewLinkTarget::LinterPage => {
            format!("[`{}`](linters/{}.md)", check.name, check.name)
        }
    }
}

fn detail_link(check: &Check) -> String {
    format!("docs/linters/{}.md", check.name)
}

fn linter_page_heading(check: &Check) -> String {
    format!("# `{}`", check.name)
}

fn generate_linter_metadata(check: &Check) -> String {
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
    lines.join("\n")
}

fn linter_page_path(manifest_dir: &Path, check: &Check) -> std::path::PathBuf {
    manifest_dir
        .join("docs/linters")
        .join(format!("{}.md", check.name))
}

fn generated_metadata_section(check: &Check) -> String {
    format!(
        "{METADATA_START}\n{}\n{METADATA_END}",
        generated_metadata_body(check)
    )
}

fn generated_metadata_body(check: &Check) -> String {
    format!("{GENERATED_COMMENT}\n{}", generate_linter_metadata(check))
}

fn update_linter_pages(manifest_dir: &Path, registry: &[Check]) {
    let pages_dir = manifest_dir.join("docs/linters");
    std::fs::create_dir_all(&pages_dir).expect("failed to create docs/linters");
    let expected_names = expected_linter_page_names(registry);

    for entry in std::fs::read_dir(&pages_dir).expect("docs/linters must be readable") {
        let entry = entry.expect("failed to read docs/linters entry");
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.extension().and_then(|extension| extension.to_str()) == Some("md")
            && !expected_names.contains(&name)
        {
            std::fs::remove_file(&path)
                .unwrap_or_else(|error| panic!("failed to remove {}: {error}", path.display()));
        }
    }

    for check in registry {
        let path = linter_page_path(manifest_dir, check);
        let heading = linter_page_heading(check);
        let metadata = generated_metadata_section(check);
        let existing = std::fs::read_to_string(&path).ok();

        let body = match existing.as_deref() {
            Some(content) if content.contains(METADATA_START) => {
                let content = replace_first_line(content, &heading);
                replace_section(
                    &content,
                    METADATA_START,
                    METADATA_END,
                    &generated_metadata_body(check),
                )
            }
            Some(content) => {
                let content = content
                    .split_once('\n')
                    .map(|(_, rest)| rest.trim_start_matches('\n'))
                    .unwrap_or_default();
                format!("{heading}\n\n{metadata}\n\n{content}")
            }
            None => format!("{heading}\n\n{metadata}\n"),
        };

        std::fs::write(&path, body).expect("failed to write dedicated linter page");
    }
}

#[test]
fn update_linter_pages_removes_stale_markdown_pages() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pages_dir = temp.path().join("docs/linters");
    std::fs::create_dir_all(&pages_dir).expect("create docs/linters");
    std::fs::write(pages_dir.join("stale.md"), "stale").expect("write stale page");
    std::fs::write(pages_dir.join("README.txt"), "keep").expect("write non-markdown file");

    update_linter_pages(temp.path(), &[]);

    assert!(!pages_dir.join("stale.md").exists());
    assert!(pages_dir.join("README.txt").exists());
}

fn replace_first_line(content: &str, heading: &str) -> String {
    content
        .split_once('\n')
        .map(|(_, rest)| format!("{heading}\n{rest}"))
        .unwrap_or_else(|| heading.to_string())
}

fn verify_linter_pages(manifest_dir: &Path, registry: &[Check]) {
    let pages_dir = manifest_dir.join("docs/linters");
    let expected_names = expected_linter_page_names(registry);
    let actual_names: BTreeSet<String> = std::fs::read_dir(&pages_dir)
        .expect("docs/linters must be readable")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|extension| extension.to_str()) == Some("md"))
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();

    assert_eq!(
        actual_names, expected_names,
        "dedicated linter pages do not match the registry; run `mise run generate`"
    );

    for check in registry {
        let path = linter_page_path(manifest_dir, check);
        let page = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let actual = extract_section(&page, METADATA_START, METADATA_END);
        let expected = strip_blank_lines(&generated_metadata_body(check));
        assert_eq!(
            actual,
            expected,
            "{} metadata is out of sync with the registry; run `mise run generate`",
            path.display()
        );
    }
}

fn expected_linter_page_names(registry: &[Check]) -> BTreeSet<String> {
    registry
        .iter()
        .map(|check| format!("{}.md", check.name))
        .collect()
}

fn detail_rows(check: &Check) -> Vec<(&'static str, String)> {
    let mut rows: Vec<(&'static str, String)> = vec![];

    if let Some(url) = check.project_url {
        rows.push(("Project", format!("[{}]({url})", check.name)));
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
    rows.push(("Scope", format!("[{scope}](../linters.md#scope-{scope})")));

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
            "adaptive — see [when does this run?](#when-does-this-run)".to_string()
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
