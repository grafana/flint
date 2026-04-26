use anyhow::{Context, Result};
use std::io;
use std::path::Path;
use std::process::Command;

use crate::registry::EditorconfigDirectiveStyle;

/// Writes a skeleton `flint.toml` in `config_dir`. Creates the directory if needed.
/// Returns `true` if the file was written, `false` if it already existed.
///
/// `exclude_managers`: when `Some`, populates `exclude_managers` in `[checks.renovate-deps]`
/// with the given list (migrated from `RENOVATE_TRACKED_DEPS_EXCLUDE`). When `None` and
/// `has_renovate` is true, writes a commented-out placeholder instead.
pub(super) fn generate_flint_toml(
    config_dir: &Path,
    base_branch: &str,
    setup_migration_version: u32,
    has_renovate: bool,
    exclude_managers: Option<&[String]>,
) -> Result<bool> {
    let toml_path = config_dir.join("flint.toml");
    if toml_path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(config_dir)?;
    let mut content = String::from("[settings]\n");
    if base_branch != "main" {
        content.push_str(&format!("base_branch = \"{base_branch}\"\n"));
    }
    content.push_str(&format!(
        "setup_migration_version = {setup_migration_version}\n"
    ));
    content.push_str("# exclude = [\"CHANGELOG\\\\.md\"]\n");
    if has_renovate {
        content.push_str("\n[checks.renovate-deps]\n");
        match exclude_managers {
            Some(managers) if !managers.is_empty() => {
                let list = managers
                    .iter()
                    .map(|m| format!("\"{m}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                content.push_str(&format!("exclude_managers = [{list}]\n"));
            }
            _ => content.push_str("# exclude_managers = []\n"),
        }
    }
    std::fs::write(&toml_path, &content)?;
    println!("  wrote {}", toml_path.display());
    Ok(true)
}

pub(crate) fn write_setup_migration_version(
    config_dir: &Path,
    base_branch: &str,
    version: u32,
) -> Result<bool> {
    let toml_path = config_dir.join("flint.toml");
    if !toml_path.exists() {
        return generate_flint_toml(config_dir, base_branch, version, false, None);
    }

    let content = std::fs::read_to_string(&toml_path)
        .with_context(|| format!("failed to read {}", toml_path.display()))?;
    let mut doc: toml_edit::DocumentMut = content.parse().context("failed to parse flint.toml")?;
    if doc.get("settings").is_none() {
        doc["settings"] = toml_edit::table();
    }
    let Some(settings) = doc.get_mut("settings").and_then(|item| item.as_table_mut()) else {
        anyhow::bail!("[settings] is not a table in {}", toml_path.display());
    };
    let current = settings
        .get("setup_migration_version")
        .and_then(|item| item.as_value())
        .and_then(|value| value.as_integer())
        .and_then(|value| u32::try_from(value).ok());
    if current == Some(version) {
        return Ok(false);
    }
    settings.insert(
        "setup_migration_version",
        toml_edit::value(i64::from(version)),
    );
    std::fs::write(&toml_path, doc.to_string())
        .with_context(|| format!("failed to write {}", toml_path.display()))?;
    Ok(true)
}

/// Generates `.rumdl.toml` in the flint config dir when rumdl is being set up.
/// Returns `true` if the file was written (or an older markdownlint variant was replaced).
pub(super) fn generate_rumdl_config(
    project_root: &Path,
    config_dir: &Path,
    line_length: u16,
) -> Result<bool> {
    const LEGACY_CONFIG_NAMES: &[&str] = &[
        ".markdownlint.json",
        ".markdownlint.jsonc",
        ".markdownlint.yaml",
        ".markdownlint.yml",
        ".markdownlint-cli2.jsonc",
        ".markdownlint-cli2.yaml",
        ".markdownlint-cli2.yml",
        ".markdownlint-cli2.cjs",
        ".markdownlint-cli2.mjs",
    ];
    let target = config_dir.join(".rumdl.toml");
    if target.exists() {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    for name in LEGACY_CONFIG_NAMES {
        let legacy = project_root.join(name);
        if legacy.exists() {
            std::fs::remove_file(&legacy)?;
            println!("  removed {} (replaced by .rumdl.toml)", legacy.display());
        }
    }
    let content = format!(
        "[MD013]\n\
         enabled = true\n\
         line-length = {line_length}\n\
         code-blocks = false\n\
         tables = false\n\
         \n\
         [MD060]\n\
         enabled = true\n\
         style = \"aligned\"\n",
    );
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

/// Removes stale v1/super-linter-era files that flint v2 no longer uses.
/// Returns the list of removed paths relative to `project_root`.
pub(super) fn remove_legacy_lint_files(
    project_root: &Path,
    config_dir: &Path,
) -> Result<Vec<String>> {
    let candidates = legacy_lint_files(project_root, config_dir);

    let mut removed = vec![];
    for path in candidates {
        if !path.exists() {
            continue;
        }
        std::fs::remove_file(&path)?;
        let rel = path
            .strip_prefix(project_root)
            .unwrap_or(&path)
            .display()
            .to_string();
        removed.push(rel);
    }
    Ok(removed)
}

pub(super) fn existing_legacy_lint_files(project_root: &Path, config_dir: &Path) -> Vec<String> {
    legacy_lint_files(project_root, config_dir)
        .into_iter()
        .filter(|path| path.exists())
        .map(|path| {
            path.strip_prefix(project_root)
                .unwrap_or(&path)
                .display()
                .to_string()
        })
        .collect()
}

fn legacy_lint_files(project_root: &Path, config_dir: &Path) -> Vec<std::path::PathBuf> {
    vec![
        project_root.join(".prettierignore"),
        project_root.join(".gitleaksignore"),
        config_dir.join("super-linter.env"),
        project_root.join(".github/config/super-linter.env"),
        project_root.join(".github/super-linter.env"),
    ]
}

/// Removes stale markdownlint MD013 directives from tracked Markdown files.
/// These long-line suppressions belong to the old markdownlint stack and should
/// disappear once rumdl owns Markdown formatting.
pub(super) fn remove_stale_markdownlint_line_length_directives(
    project_root: &Path,
) -> Result<Vec<String>> {
    let mut changed_files = vec![];
    for rel in tracked_files_for_patterns(project_root, &[&["*.md"]])? {
        let path = project_root.join(&rel);
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let updated = strip_stale_markdownlint_md013_directives(&content);
        if updated == content {
            continue;
        }
        std::fs::write(&path, updated)?;
        changed_files.push(rel.to_string());
    }
    Ok(changed_files)
}

pub(super) fn stale_markdownlint_line_length_directive_files(
    project_root: &Path,
) -> Result<Vec<String>> {
    stale_transformed_files(
        project_root,
        &[&["*.md"]],
        strip_stale_markdownlint_md013_directives,
    )
}

fn tracked_files_for_patterns(project_root: &Path, patterns: &[&[&str]]) -> Result<Vec<String>> {
    let mut tracked_files = std::collections::BTreeSet::new();
    for group in patterns {
        for pattern in *group {
            let output = Command::new("git")
                .args(["ls-files", "--", pattern])
                .current_dir(project_root)
                .output()
                .with_context(|| format!("failed to list tracked files for {pattern}"))?;
            if !output.status.success() {
                anyhow::bail!("git ls-files failed while scanning {pattern}");
            }
            let stdout =
                String::from_utf8(output.stdout).context("git ls-files output was not UTF-8")?;
            tracked_files.extend(
                stdout
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(str::to_string),
            );
        }
    }
    Ok(tracked_files.into_iter().collect())
}

/// Removes stale editorconfig-checker suppressions from tracked files whose
/// line length is now delegated through root `.editorconfig`.
pub(super) fn remove_stale_editorconfig_checker_directives(
    project_root: &Path,
    delegated_sections: &[(&[&str], EditorconfigDirectiveStyle)],
) -> Result<Vec<String>> {
    let mut changed_files = vec![];
    for (patterns, directive_style) in delegated_sections {
        let tracked_files = tracked_files_for_patterns(project_root, &[*patterns])?;
        for rel in tracked_files {
            let path = project_root.join(rel.as_str());
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let updated = strip_stale_editorconfig_checker_directives(&content, *directive_style);
            if updated == content {
                continue;
            }
            std::fs::write(&path, updated)?;
            changed_files.push(rel.to_string());
        }
    }
    changed_files.sort();
    changed_files.dedup();
    Ok(changed_files)
}

pub(super) fn stale_editorconfig_checker_directive_files(
    project_root: &Path,
    delegated_sections: &[(&[&str], EditorconfigDirectiveStyle)],
) -> Result<Vec<String>> {
    let mut changed_files = vec![];
    for (patterns, directive_style) in delegated_sections {
        changed_files.extend(stale_transformed_files(
            project_root,
            &[*patterns],
            |content| strip_stale_editorconfig_checker_directives(content, *directive_style),
        )?);
    }
    changed_files.sort();
    changed_files.dedup();
    Ok(changed_files)
}

fn stale_transformed_files<F>(
    project_root: &Path,
    patterns: &[&[&str]],
    transform: F,
) -> Result<Vec<String>>
where
    F: Fn(&str) -> String,
{
    let tracked_files = tracked_files_for_patterns(project_root, patterns)?;
    let mut changed_files = vec![];
    for rel in tracked_files {
        let path = project_root.join(rel.as_str());
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        if transform(&content) != content {
            changed_files.push(rel);
        }
    }
    Ok(changed_files)
}

fn strip_stale_markdownlint_md013_directives(content: &str) -> String {
    let mut kept = Vec::with_capacity(content.lines().count());
    let had_trailing_newline = content.ends_with('\n');

    for line in content.lines() {
        if is_stale_markdownlint_md013_directive(line) {
            continue;
        }
        kept.push(line);
    }

    let mut updated = kept.join("\n");
    if had_trailing_newline {
        updated.push('\n');
    }
    updated
}

fn is_stale_markdownlint_md013_directive(line: &str) -> bool {
    let trimmed = line.trim();
    matches!(
        trimmed,
        "<!-- markdownlint-disable MD013 -->"
            | "<!-- markdownlint-enable MD013 -->"
            | "<!-- markdownlint-disable-line MD013 -->"
            | "<!-- markdownlint-disable-next-line MD013 -->"
            | "<!-- markdownlint-disable-file MD013 -->"
    )
}

fn strip_stale_editorconfig_checker_directives(
    content: &str,
    directive_style: EditorconfigDirectiveStyle,
) -> String {
    let had_trailing_newline = content.ends_with('\n');
    let mut kept = Vec::with_capacity(content.lines().count());

    for line in content.lines() {
        let trimmed = line.trim();
        if is_stale_editorconfig_checker_block_directive(trimmed, directive_style) {
            continue;
        }

        let mut updated = line.to_string();
        for marker in stale_editorconfig_checker_inline_markers(directive_style) {
            if let Some(idx) = updated.find(marker) {
                updated.truncate(idx);
                updated = updated.trim_end().to_string();
            }
        }
        kept.push(updated);
    }

    let mut updated = kept.join("\n");
    if had_trailing_newline {
        updated.push('\n');
    }
    updated
}

fn is_stale_editorconfig_checker_block_directive(
    trimmed: &str,
    directive_style: EditorconfigDirectiveStyle,
) -> bool {
    match directive_style {
        EditorconfigDirectiveStyle::Html => matches!(
            trimmed,
            "<!-- editorconfig-checker-disable -->"
                | "<!-- editorconfig-checker-enable -->"
                | "<!-- editorconfig-checker-disable-file -->"
        ),
        EditorconfigDirectiveStyle::Slash | EditorconfigDirectiveStyle::Hash => false,
    }
}

fn stale_editorconfig_checker_inline_markers(
    directive_style: EditorconfigDirectiveStyle,
) -> &'static [&'static str] {
    match directive_style {
        EditorconfigDirectiveStyle::Html => &["<!-- editorconfig-checker-disable-line -->"],
        EditorconfigDirectiveStyle::Slash | EditorconfigDirectiveStyle::Hash => &[],
    }
}

/// Generates or updates `.editorconfig` in the project root.
///
/// Existing explicit global `[*]` `max_line_length` settings are left
/// untouched. When a root `[*]` section exists without a line-length
/// setting, flint adds one there; otherwise it appends a minimal `[*]`
/// section.
pub(super) fn generate_editorconfig(project_root: &Path, line_length: u16) -> Result<bool> {
    let target = project_root.join(".editorconfig");
    let content = match std::fs::read_to_string(&target) {
        Ok(content) => content,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let content = format!(
                "root = true\n\
                 \n\
                 [*]\n\
                 charset = utf-8\n\
                 end_of_line = lf\n\
                 indent_style = space\n\
                 indent_size = 2\n\
                 insert_final_newline = true\n\
                 trim_trailing_whitespace = true\n\
                 max_line_length = {line_length}\n",
            );
            std::fs::write(&target, content)?;
            println!("  wrote {}", target.display());
            return Ok(true);
        }
        Err(e) => return Err(e).with_context(|| format!("failed to read {}", target.display())),
    };

    if editorconfig_has_global_line_length(&content) {
        return Ok(false);
    }

    let updated = add_editorconfig_global_line_length(&content, line_length);
    if updated == content {
        return Ok(false);
    }
    std::fs::write(&target, updated)?;
    println!(
        "  patched {} — set max_line_length = {line_length}",
        target.display()
    );
    Ok(true)
}

fn editorconfig_has_global_line_length(content: &str) -> bool {
    let mut in_global = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_global = trimmed == "[*]";
            continue;
        }
        if in_global && trimmed.starts_with("max_line_length") {
            return true;
        }
    }
    false
}

fn add_editorconfig_global_line_length(content: &str, line_length: u16) -> String {
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let had_trailing_newline = content.ends_with('\n');
    let Some(section_start) = lines.iter().position(|line| line.trim() == "[*]") else {
        let mut updated = content.to_string();
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        if !updated.ends_with("\n\n") {
            updated.push('\n');
        }
        updated.push_str(&format!("[*]\nmax_line_length = {line_length}\n"));
        return updated;
    };

    let section_end = lines
        .iter()
        .enumerate()
        .skip(section_start + 1)
        .find_map(|(idx, line)| {
            let trimmed = line.trim();
            (trimmed.starts_with('[') && trimmed.ends_with(']')).then_some(idx)
        })
        .unwrap_or(lines.len());
    let insert_at = lines
        .iter()
        .enumerate()
        .take(section_end)
        .skip(section_start + 1)
        .find_map(|(idx, line)| (line.trim_start().starts_with("indent_size")).then_some(idx + 1))
        .unwrap_or(section_end);

    lines.insert(insert_at, format!("max_line_length = {line_length}"));
    let mut updated = lines.join("\n");
    if had_trailing_newline {
        updated.push('\n');
    }
    updated
}

pub(super) fn disable_editorconfig_line_length_for_patterns(
    project_root: &Path,
    sections: &[(&'static [&'static str], &'static str)],
) -> Result<Vec<String>> {
    if sections.is_empty() {
        return Ok(vec![]);
    }

    let path = project_root.join(".editorconfig");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let had_trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let mut changed_sections = vec![];

    for (patterns, comment) in sections {
        let header = editorconfig_section_header(patterns);
        let comment_line = format!("# {comment}");
        if let Some(section_start) = lines.iter().position(|line| line.trim() == header) {
            let section_end = lines
                .iter()
                .enumerate()
                .skip(section_start + 1)
                .find_map(|(idx, line)| {
                    let trimmed = line.trim();
                    (trimmed.starts_with('[') && trimmed.ends_with(']')).then_some(idx)
                })
                .unwrap_or(lines.len());
            let section_lines = &lines[section_start + 1..section_end];
            if section_lines
                .iter()
                .any(|line| line.trim_start().starts_with("max_line_length"))
            {
                continue;
            }
            let mut insert = vec![];
            if !section_lines.iter().any(|line| line.trim() == comment_line) {
                insert.push(comment_line);
            }
            insert.push("max_line_length = off".to_string());
            lines.splice(section_end..section_end, insert);
            changed_sections.push(header);
            continue;
        }

        if !lines.is_empty() && !lines.last().is_some_and(|line| line.is_empty()) {
            lines.push(String::new());
        }
        lines.push(header.clone());
        lines.push(comment_line);
        lines.push("max_line_length = off".to_string());
        changed_sections.push(header);
    }

    if changed_sections.is_empty() {
        return Ok(vec![]);
    }

    let mut updated = lines.join("\n");
    if had_trailing_newline {
        updated.push('\n');
    }
    std::fs::write(&path, updated)?;
    Ok(changed_sections)
}

fn editorconfig_section_header(patterns: &[&str]) -> String {
    if patterns.len() == 1 {
        format!("[{}]", patterns[0])
    } else {
        format!("[{{{}}}]", patterns.join(","))
    }
}

/// Generates `.yamllint.yml` in the flint config dir when yaml-lint is being set up.
pub(super) fn generate_yamllint_config(config_dir: &Path, line_length: u16) -> Result<bool> {
    let target = config_dir.join(".yamllint.yml");
    if target.exists() {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = [
        "extends: relaxed",
        "",
        "rules:",
        "  document-start: disable",
        "  line-length:",
        &format!("    max: {line_length}"),
        "  indentation:",
        "    spaces: 2",
        "",
    ]
    .join("\n");
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

/// Generates `.taplo.toml` in the flint config dir when taplo is being set up.
pub(super) fn generate_taplo_config(config_dir: &Path, line_length: u16) -> Result<bool> {
    const SUPPORTED_CONFIG_NAMES: &[&str] = &[".taplo.toml"];
    const LEGACY_CONFIG_NAMES: &[&str] = &["taplo.toml"];
    if SUPPORTED_CONFIG_NAMES
        .iter()
        .map(|name| config_dir.join(name))
        .any(|path| path.exists())
        || LEGACY_CONFIG_NAMES
            .iter()
            .map(|name| config_dir.join(name))
            .any(|path| path.exists())
    {
        return Ok(false);
    }
    let target = config_dir.join(".taplo.toml");
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = [
        "[formatting]".to_string(),
        format!("column_width = {line_length}"),
        "indent_string = \"  \"".to_string(),
    ]
    .join("\n")
        + "\n";
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

/// Generates `rustfmt.toml` in the flint config dir when cargo-fmt is being set up.
pub(super) fn generate_rustfmt_config(config_dir: &Path, line_length: u16) -> Result<bool> {
    let target = config_dir.join("rustfmt.toml");
    if target.exists() {
        return Ok(false);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = format!("max_width = {line_length}\n");
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

/// Generates root `biome.jsonc` when biome is being set up and no
/// existing supported config is present.
///
/// Flint writes explicit space indentation to avoid Biome's default tab
/// formatting surprising consumers during rollout.
pub(super) fn generate_biome_config(project_root: &Path) -> Result<bool> {
    let target = project_root.join("biome.jsonc");
    if target.exists() {
        return Ok(false);
    }
    let legacy = project_root.join("biome.json");
    if legacy.exists() {
        std::fs::rename(&legacy, &target)?;
        println!("  moved {} -> {}", legacy.display(), target.display());
        return Ok(true);
    }
    let content = [
        "{",
        "  \"formatter\": {",
        "    \"indentStyle\": \"space\",",
        "    \"indentWidth\": 2",
        "  }",
        "}",
        "",
    ]
    .join("\n");
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}
