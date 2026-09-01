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
    pub fn load_or_create(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            Ok(toml::from_str(&content)?)
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, toml::to_string_pretty(&config)?)?;
            Ok(config)
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
