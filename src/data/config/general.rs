use std::path::{Path, PathBuf};

use super::config_dir;
use crate::data::schema;

const GENERAL_FILE_NAME: &str = "general.toml";
pub use schema::{FolderStylePresetConfig, GeneralConfig};

pub fn general_toml_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(GENERAL_FILE_NAME)
}

pub fn default_general_config() -> GeneralConfig {
    schema::default_general_config()
}

pub fn normalize_general_config(data: GeneralConfig) -> GeneralConfig {
    schema::normalize_general_config(data)
}

pub fn read_general_toml_path(path: &Path) -> Result<GeneralConfig, String> {
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        return toml::from_str::<GeneralConfig>(&content)
            .map(normalize_general_config)
            .map_err(|err| err.to_string());
    }

    Ok(default_general_config())
}

pub fn write_general_toml_path(path: &Path, data: &GeneralConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let content = toml::to_string_pretty(data).map_err(|err| err.to_string())?;
    std::fs::write(path, content).map_err(|err| err.to_string())
}
