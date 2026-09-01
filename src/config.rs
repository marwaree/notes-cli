use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub notes_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            notes_dir: shellexpand::tilde("~/Notes").to_string(),
        }
    }
}

impl Config {
    pub fn load_or_create(path: &Path) -> Result<Config> {
        if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to load config from {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse TOML syntax in '{}'", path.display()))?;
            Ok(config)
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }
            fs::write(path, toml::to_string_pretty(&config)?)
                .with_context(|| format!("Failed to write config to {}", path.display()))?;
            Ok(config)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        fs::write(path, toml::to_string_pretty(self)?)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }
}
