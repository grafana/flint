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
    pub setup_migration_version: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_branch: "main".to_string(),
            exclude: vec![],
            setup_migration_version: crate::setup::V2_BASELINE_SETUP_VERSION,
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
    /// How many lines from the top of each file to search. Default: 5.
    pub lines_to_check: usize,
}

impl Default for LicenseHeaderConfig {
    fn default() -> Self {
        Self {
            text: String::new(),
            patterns: vec![],
            lines_to_check: 5,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_setup_migration_version_defaults_to_v2_baseline() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("flint.toml"), "[settings]\n").unwrap();

        let cfg = load(tmp.path()).unwrap();

        assert_eq!(
            cfg.settings.setup_migration_version,
            crate::setup::V2_BASELINE_SETUP_VERSION
        );
    }
}
