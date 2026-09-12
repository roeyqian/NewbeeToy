use std::path::{Path, PathBuf};

use super::config_dir;
use crate::data::schema;

const SYSTEM_FILE_NAME: &str = "system.toml";
pub use schema::{SystemConfig, SystemPresetConfig};

pub fn system_toml_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(SYSTEM_FILE_NAME)
}

pub fn read_system_toml_path(path: &Path) -> Result<SystemConfig, String> {
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        return toml::from_str::<SystemConfig>(&content).map_err(|err| err.to_string());
    }

    Ok(SystemConfig::default())
}

pub fn write_system_toml_path(path: &Path, data: &SystemConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let content = toml::to_string_pretty(data).map_err(|err| err.to_string())?;
    std::fs::write(path, content).map_err(|err| err.to_string())
}
