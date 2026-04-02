use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
    pub checks: ChecksConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub base_branch: String,
    pub exclude: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_branch: "main".to_string(),
            exclude: None,
        }
    }
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(default)]
pub struct ChecksConfig {
    pub lychee: LycheeConfig,
    #[serde(rename = "renovate-deps")]
    pub renovate_deps: RenovateDepsConfig,
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
    pub exclude_managers: Vec<String>,
}

pub fn load(project_root: &Path) -> Result<Config> {
    let path = project_root.join("flint.toml");
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)?;
    let cfg: Config = toml::from_str(&text)?;
    Ok(cfg)
}
