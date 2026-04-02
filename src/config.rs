use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
}

#[derive(Debug, Deserialize)]
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

pub fn load(project_root: &Path) -> Result<Config> {
    let path = project_root.join("flint.toml");
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)?;
    let cfg: Config = toml::from_str(&text)?;
    Ok(cfg)
}
