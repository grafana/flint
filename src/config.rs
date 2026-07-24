use anyhow::Result;
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;
use std::path::Path;

use crate::registry;

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub settings: Settings,
    pub checks: ChecksConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub base_branch: String,
    pub exclude: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_branch: "main".to_string(),
            exclude: vec![],
        }
    }
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct ChecksConfig {
    pub lychee: LycheeConfig,
    // The alias allows the underscore form used in env var keys alongside the
    // hyphenated form used in flint.toml.
    #[serde(rename = "renovate-deps", alias = "renovate_deps")]
    pub renovate_deps: RenovateDepsConfig,
    #[serde(rename = "license-header", alias = "license_header")]
    pub license_header: LicenseHeaderConfig,
    #[serde(rename = "google-java-format", alias = "google_java_format")]
    pub google_java_format: GoogleJavaFormatConfig,
    #[serde(rename = "regex-replace", alias = "regex_replace")]
    pub regex_replace: RegexReplaceConfig,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct LycheeConfig {
    pub config: Option<String>,
    pub check_all_local: bool,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct RenovateDepsConfig {
    // Env var: FLINT_RENOVATE_DEPS_EXCLUDE_MANAGERS (JSON array, e.g. '["npm"]')
    pub exclude_managers: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct LicenseHeaderConfig {
    /// The text that must appear within the first `lines_to_check` lines of each file.
    /// When empty (default), the check is disabled.
    pub text: String,
    /// Glob patterns for files to check (e.g. `["*.java", "*.kt"]`).
    pub patterns: Vec<String>,
    /// Glob patterns excluded after `patterns` are matched. Uses the same glob
    /// syntax as `settings.exclude`.
    pub exclude: Vec<String>,
    /// How many lines from the top of each file to search. Default: 5.
    pub lines_to_check: usize,
}

impl Default for LicenseHeaderConfig {
    fn default() -> Self {
        Self {
            text: String::new(),
            patterns: vec![],
            exclude: vec![],
            lines_to_check: 5,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct GoogleJavaFormatConfig {
    /// Restrict formatting to paths matching these globs. An empty list uses the
    /// check's registered patterns.
    pub patterns: Vec<String>,
    /// Glob patterns excluded after `patterns` are matched. Uses the same glob
    /// syntax as `settings.exclude`.
    pub exclude: Vec<String>,
    /// Keep the behavior used by Spotless and google-java-format 1.35, which
    /// does not reflow long strings unless explicitly requested.
    pub skip_reflowing_long_strings: bool,
    pub skip_sorting_imports: bool,
    pub skip_removing_unused_imports: bool,
    pub skip_javadoc_formatting: bool,
    pub aosp: bool,
    /// Comment marker pairs whose regions should be restored after formatting.
    /// This is useful for repositories using formatter-off directives that GJF
    /// itself does not understand.
    pub off_on_markers: Vec<OffOnMarkerConfig>,
}

impl Default for GoogleJavaFormatConfig {
    fn default() -> Self {
        Self {
            patterns: vec![],
            exclude: vec![],
            skip_reflowing_long_strings: true,
            skip_sorting_imports: false,
            skip_removing_unused_imports: false,
            skip_javadoc_formatting: false,
            aosp: false,
            off_on_markers: vec![],
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct OffOnMarkerConfig {
    pub off: String,
    pub on: String,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct RegexReplaceConfig {
    /// Restrict rewriting to paths matching these globs. An empty list uses
    /// all files in the Flint file list.
    pub patterns: Vec<String>,
    /// Glob patterns excluded after `patterns` are matched. Uses the same glob
    /// syntax as `settings.exclude`.
    pub exclude: Vec<String>,
    /// Ordered rule sets. Each set has its own scope and defaults.
    pub sets: Vec<RegexReplaceSetConfig>,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct RegexReplaceSetConfig {
    /// Human-readable name used in diagnostics and documentation.
    pub name: String,
    /// Additional file scope for this set. Empty means all files selected by
    /// the parent check.
    pub patterns: Vec<String>,
    /// Additional exclusions for this set, using the same glob syntax as
    /// `settings.exclude`.
    pub exclude: Vec<String>,
    /// Defaults inherited by rules in this set. A rule can override them.
    pub replacement: Option<String>,
    /// Optional line regexes controlling where this set's `add_lines` are
    /// inserted.
    pub add_lines_before_pattern: Option<String>,
    pub add_lines_fallback_after_pattern: Option<String>,
    /// Ignore source lines matching this regex before applying this set.
    pub skip_line_pattern: Option<String>,
    /// Configured regions to ignore while applying this set. This is generic
    /// region handling; it does not assume a particular comment syntax.
    pub ignore_regions: Vec<RegexReplaceIgnoreRegionConfig>,
    pub rules: Vec<RegexReplaceRuleConfig>,
    /// Rules that derive a replacement rule from a line matched by
    /// `source_pattern`.
    pub derived_rules: Vec<DerivedRegexReplaceRuleConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegexReplaceIgnoreRegionConfig {
    /// Regex matching the line where an ignored region starts.
    pub start_pattern: String,
    /// Regex matching the line where an ignored region ends.
    pub end_pattern: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegexReplaceRuleConfig {
    /// Regex matching text to replace. Capture groups may be used by
    /// `replacement` and `add_lines`.
    pub pattern: String,
    /// Replacement text passed to regex capture expansion. When omitted, the
    /// set's replacement default is used, then `$0`.
    #[serde(default)]
    pub replacement: Option<String>,
    /// Lines to add when this rule matches, also supporting regex capture
    /// expansion.
    #[serde(default)]
    pub add_lines: Vec<String>,
    /// Only apply this rule when the file contains a match for this regex.
    #[serde(default)]
    pub content_pattern: Option<String>,
    #[serde(default)]
    pub line_exclude_pattern: Option<String>,
    #[serde(default)]
    pub file_pattern: Option<String>,
    #[serde(default)]
    pub content_exclude_pattern: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DerivedRegexReplaceRuleConfig {
    /// Regex matched against source lines. Named captures can be referenced as
    /// `{name}` in the other fields.
    pub source_pattern: String,
    /// Regex for replacement text. `{name}` placeholders are replaced with
    /// escaped source captures before this regex is compiled.
    pub pattern: String,
    /// Overrides the set replacement default; otherwise `$0` is used.
    #[serde(default)]
    pub replacement: Option<String>,
    /// Lines to add. `{name}` source placeholders and `$1`, `$2`, ... match
    /// captures may be used.
    #[serde(default)]
    pub add_lines: Vec<String>,
    #[serde(default)]
    pub source_exclude_pattern: Option<String>,
}

/// Builds env-var prefix → figment key-path mappings for every check in the registry.
/// e.g. "lychee"        → ("lychee_",        "checks.lychee.")
///      "renovate-deps" → ("renovate_deps_",  "checks.renovate_deps.")
///      "ruff-format"   → ("ruff_format_",    "checks.ruff_format.")
/// Sorted longest-prefix-first so "ruff_fmt_" is matched before "ruff_".
fn check_env_sections() -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = registry::builtin()
        .into_iter()
        .map(|c| {
            let n = c.name.replace('-', "_");
            (format!("{n}_"), format!("checks.{n}."))
        })
        .collect();
    // Dedup by prefix (multiple checks can share a name after normalisation is unlikely,
    // but be safe) then sort longest-first to avoid short prefixes shadowing longer ones.
    sections.sort_by_key(|section| std::cmp::Reverse(section.0.len()));
    sections.dedup_by(|a, b| a.0 == b.0);
    sections
}

pub fn load(config_dir: &Path) -> Result<Config> {
    let sections = check_env_sections();
    let cfg: Config = Figment::new()
        .merge(Toml::file(config_dir.join("flint.toml")))
        // Flat FLINT_ env vars, no double-underscore separators:
        //   FLINT_BASE_BRANCH, FLINT_EXCLUDE          → settings.*
        //   FLINT_LYCHEE_CONFIG, FLINT_LYCHEE_*       → checks.lychee.*
        //   FLINT_RENOVATE_DEPS_EXCLUDE_MANAGERS       → checks.renovate_deps.*
        // New native checks added to the registry get env support automatically.
        .merge(Env::prefixed("FLINT_").map(move |k| {
            let k = k.as_str();
            for (prefix, namespace) in &sections {
                if let Some(rest) = k.strip_prefix(prefix.as_str()) {
                    return format!("{namespace}{rest}").into();
                }
            }
            format!("settings.{k}").into()
        }))
        .extract()?;
    Ok(cfg)
}
