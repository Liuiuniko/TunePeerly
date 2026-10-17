use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const APP_DIR_NAME: &str = "MultiDeviceMusicPlayer";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", default)]
pub struct StartupConfig {
    pub database_path: String,
    pub listen_port: u16,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            database_path: "library.db".into(),
            listen_port: 0,
        }
    }
}

pub fn app_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(APP_DIR_NAME)
}

pub fn config_path(app_dir: &Path) -> PathBuf {
    app_dir.join("config.toml")
}

pub fn load_config(app_dir: &Path) -> AppResult<StartupConfig> {
    fs::create_dir_all(app_dir)?;
    let path = config_path(app_dir);
    if !path.exists() {
        let config = StartupConfig::default();
        save_config(app_dir, &config)?;
        return Ok(config);
    }
    let text = fs::read_to_string(&path)?;
    let value: toml::Value = toml::from_str(&text).unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
    let mut config = StartupConfig::default();
    let mut missing = false;
    if let Some(table) = value.as_table() {
        if let Some(path) = table.get("database_path").and_then(|item| item.as_str()) {
            config.database_path = path.to_string();
        } else {
            missing = true;
        }
        if let Some(port) = table.get("listen_port").and_then(|item| item.as_integer()) {
            config.listen_port = port.clamp(0, u16::MAX as i64) as u16;
        } else {
            missing = true;
        }
    } else {
        missing = true;
    }
    if missing {
        save_config(app_dir, &config)?;
    }
    Ok(config)
}

pub fn save_config(app_dir: &Path, config: &StartupConfig) -> AppResult<()> {
    fs::create_dir_all(app_dir)?;
    let text = toml::to_string_pretty(config)?;
    fs::write(config_path(app_dir), text)?;
    Ok(())
}

pub fn resolve_database_path(app_dir: &Path, config: &StartupConfig) -> PathBuf {
    let path = PathBuf::from(&config.database_path);
    if path.is_absolute() {
        path
    } else {
        app_dir.join(path)
    }
}

pub fn validate_database_path(path: &str) -> AppResult<()> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::Message("Database path is empty.".into()));
    }
    Ok(())
}
