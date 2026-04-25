use anyhow::{Context, Result};
use std::io;
use std::path::Path;
use std::process::Command;

/// Writes a skeleton `flint.toml` in `config_dir`. Creates the directory if needed.
/// Returns `true` if the file was written, `false` if it already existed.
///
/// `exclude_managers`: when `Some`, populates `exclude_managers` in `[checks.renovate-deps]`
/// with the given list (migrated from `RENOVATE_TRACKED_DEPS_EXCLUDE`). When `None` and
/// `has_renovate` is true, writes a commented-out placeholder instead.
pub(super) fn generate_flint_toml(
    config_dir: &Path,
    base_branch: &str,
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
         tables = false\n",
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
    let candidates = [
        project_root.join(".prettierignore"),
        project_root.join(".gitleaksignore"),
        config_dir.join("super-linter.env"),
        project_root.join(".github/config/super-linter.env"),
        project_root.join(".github/super-linter.env"),
    ];

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

/// Removes stale markdownlint MD013 directives from tracked Markdown files.
/// These long-line suppressions belong to the old markdownlint stack and should
/// disappear once rumdl owns Markdown formatting.
pub(super) fn remove_stale_markdownlint_line_length_directives(
    project_root: &Path,
) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["ls-files", "--", "*.md"])
        .current_dir(project_root)
        .output()
        .context("failed to list tracked Markdown files")?;
    if !output.status.success() {
        anyhow::bail!("git ls-files failed while scanning Markdown files");
    }

    let stdout = String::from_utf8(output.stdout).context("git ls-files output was not UTF-8")?;
    let mut changed_files = vec![];
    for rel in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let path = project_root.join(rel);
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

/// Updates an existing editorconfig-checker config so file types owned by
/// formatter-style linters are excluded from ec overlap checks.
///
/// Checks both the project root and `config_dir`, updating the first config
/// file that exists. Returns `true` when a config was changed.
pub(super) fn exclude_formatter_owned_files_from_editorconfig_checker(
    project_root: &Path,
    config_dir: &Path,
) -> Result<bool> {
    const EXCLUDES: &[&str] = &[".*\\.md$", ".*\\.yml$", ".*\\.yaml$"];
    let candidates = [
        config_dir.join(".editorconfig-checker.json"),
        project_root.join(".editorconfig-checker.json"),
    ];

    for path in candidates {
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let mut value: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let Some(obj) = value.as_object_mut() else {
            anyhow::bail!("{} is not a JSON object", path.display());
        };

        let key = if obj.contains_key("Exclude") {
            "Exclude"
        } else if obj.contains_key("exclude") {
            "exclude"
        } else {
            "Exclude"
        };

        let entry = obj
            .entry(key.to_string())
            .or_insert_with(|| serde_json::Value::Array(vec![]));
        let Some(items) = entry.as_array_mut() else {
            anyhow::bail!("{} field in {} is not an array", key, path.display());
        };

        let mut changed = false;
        for pattern in EXCLUDES {
            if items
                .iter()
                .any(|v| v.as_str().is_some_and(|s| s == *pattern))
            {
                continue;
            }

            items.push(serde_json::Value::String((*pattern).to_string()));
            changed = true;
        }

        if !changed {
            return Ok(false);
        }
        let updated = serde_json::to_string_pretty(&value)? + "\n";
        std::fs::write(&path, updated)?;
        return Ok(true);
    }

    Ok(false)
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
