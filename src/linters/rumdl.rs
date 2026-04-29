use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::registry::{InitHookContext, StaticLinter};

pub(crate) static LINTER: StaticLinter = StaticLinter::with_init_hook("rumdl", init);

pub(crate) fn init(ctx: &dyn InitHookContext) -> Result<bool> {
    generate_config(ctx.project_root(), ctx.config_dir(), ctx.line_length())
}

pub(crate) fn generate_config(
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
    let content = converted_legacy_markdownlint_config(project_root)?
        .unwrap_or_else(|| default_config(line_length));
    for name in LEGACY_CONFIG_NAMES {
        let legacy = project_root.join(name);
        if legacy.exists() {
            std::fs::remove_file(&legacy)?;
            println!("  removed {} (replaced by .rumdl.toml)", legacy.display());
        }
    }
    std::fs::write(&target, content)?;
    println!("  wrote {}", target.display());
    Ok(true)
}

fn default_config(line_length: u16) -> String {
    format!(
        "[MD013]\n\
         enabled = true\n\
         line-length = {line_length}\n\
         code-blocks = false\n\
         tables = false\n\
         \n\
         [MD060]\n\
         enabled = true\n\
         style = \"aligned\"\n",
    )
}

fn converted_legacy_markdownlint_config(project_root: &Path) -> Result<Option<String>> {
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

    for name in LEGACY_CONFIG_NAMES {
        let path = project_root.join(name);
        if !path.exists() {
            continue;
        }
        if let Some(config) = parse_legacy_markdownlint_config(&path)? {
            return Ok(Some(render_rumdl_config_from_legacy(&config)));
        }
    }

    Ok(None)
}

#[derive(Debug, Default, Deserialize)]
struct LegacyMarkdownlintConfig {
    #[serde(rename = "line-length", alias = "MD013")]
    line_length: Option<LegacyRuleSetting<LegacyLineLengthRule>>,
    #[serde(rename = "ul-style", alias = "MD004")]
    ul_style: Option<LegacyRuleSetting<EmptyRule>>,
    #[serde(rename = "no-duplicate-heading", alias = "MD024")]
    no_duplicate_heading: Option<LegacyRuleSetting<LegacyNoDuplicateHeadingRule>>,
    #[serde(rename = "ol-prefix", alias = "MD029")]
    ol_prefix: Option<LegacyRuleSetting<LegacyOlPrefixRule>>,
    #[serde(rename = "no-inline-html", alias = "MD033")]
    no_inline_html: Option<LegacyRuleSetting<EmptyRule>>,
    #[serde(rename = "fenced-code-language", alias = "MD040")]
    fenced_code_language: Option<LegacyRuleSetting<EmptyRule>>,
    #[serde(rename = "no-trailing-punctuation", alias = "MD026")]
    no_trailing_punctuation: Option<LegacyRuleSetting<LegacyNoTrailingPunctuationRule>>,
    #[serde(rename = "MD041")]
    md041: Option<LegacyRuleSetting<EmptyRule>>,
    #[serde(rename = "MD059")]
    md059: Option<LegacyRuleSetting<EmptyRule>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LegacyRuleSetting<T> {
    Bool(bool),
    Config(T),
}

#[derive(Debug, Default, Deserialize)]
struct EmptyRule {}

#[derive(Debug, Default, Deserialize)]
struct LegacyLineLengthRule {
    #[serde(rename = "line_length")]
    line_length: Option<u16>,
    #[serde(rename = "code_blocks")]
    code_blocks: Option<bool>,
    tables: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct LegacyNoDuplicateHeadingRule {
    #[serde(rename = "siblings_only")]
    siblings_only: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct LegacyOlPrefixRule {
    style: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LegacyNoTrailingPunctuationRule {
    punctuation: Option<String>,
}

fn parse_legacy_markdownlint_config(path: &Path) -> Result<Option<LegacyMarkdownlintConfig>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();

    let parsed = match ext {
        "json" | "jsonc" => json5::from_str::<LegacyMarkdownlintConfig>(&content).ok(),
        "yaml" | "yml" => serde_yaml::from_str::<LegacyMarkdownlintConfig>(&content).ok(),
        _ => None,
    };
    Ok(parsed)
}

fn render_rumdl_config_from_legacy(config: &LegacyMarkdownlintConfig) -> String {
    let mut out = String::new();
    let mut global_disable = vec![];

    append_global_disable(&mut global_disable, "line-length", &config.line_length);
    append_global_disable(&mut global_disable, "ul-style", &config.ul_style);
    append_global_disable(
        &mut global_disable,
        "no-inline-html",
        &config.no_inline_html,
    );
    append_global_disable(
        &mut global_disable,
        "fenced-code-language",
        &config.fenced_code_language,
    );
    append_global_disable(&mut global_disable, "MD041", &config.md041);
    append_global_disable(&mut global_disable, "MD059", &config.md059);

    if !global_disable.is_empty() {
        out.push_str("[global]\n");
        out.push_str("disable = [");
        out.push_str(
            &global_disable
                .iter()
                .map(|rule| format!("\"{rule}\""))
                .collect::<Vec<_>>()
                .join(", "),
        );
        out.push_str("]\n");
    }

    if let Some(LegacyRuleSetting::Config(rule)) = &config.line_length {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("[MD013]\n");
        if let Some(line_length) = rule.line_length {
            out.push_str("enabled = true\n");
            out.push_str(&format!("line-length = {line_length}\n"));
        }
        if let Some(code_blocks) = rule.code_blocks {
            out.push_str(&format!("code-blocks = {code_blocks}\n"));
        }
        if let Some(tables) = rule.tables {
            out.push_str(&format!("tables = {tables}\n"));
        }
    }

    if let Some(LegacyRuleSetting::Config(rule)) = &config.no_duplicate_heading
        && rule.siblings_only.is_some()
    {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("[no-duplicate-heading]\n");
        out.push_str(&format!(
            "siblings-only = {}\n",
            rule.siblings_only.unwrap_or(false)
        ));
    }

    if let Some(LegacyRuleSetting::Config(rule)) = &config.no_trailing_punctuation
        && let Some(punctuation) = &rule.punctuation
    {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("[no-trailing-punctuation]\n");
        out.push_str(&format!("punctuation = \"{punctuation}\"\n"));
    }

    if let Some(LegacyRuleSetting::Config(rule)) = &config.ol_prefix
        && let Some(style) = &rule.style
    {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("[ol-prefix]\n");
        out.push_str(&format!("style = \"{style}\"\n"));
    }

    out
}

fn append_global_disable<T>(
    global_disable: &mut Vec<&'static str>,
    rule_name: &'static str,
    setting: &Option<LegacyRuleSetting<T>>,
) {
    if matches!(setting, Some(LegacyRuleSetting::Bool(false))) {
        global_disable.push(rule_name);
    }
}
