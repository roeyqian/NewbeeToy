use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::config_dir;

const SYSTEM_FILE_NAME: &str = "system.dat";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemDat {
    #[serde(default)]
    pub presets: BTreeMap<String, SystemPresetDat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemPresetDat {
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
}

pub fn system_dat_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(SYSTEM_FILE_NAME)
}

pub fn read_system_dat_path(path: &Path) -> Result<SystemDat, String> {
    if !path.exists() {
        return Ok(SystemDat::default());
    }

    let content = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    toml::from_str::<SystemDat>(&content).map_err(|err| err.to_string())
}

pub fn write_system_dat_path(path: &Path, data: &SystemDat) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let content = toml::to_string_pretty(data).map_err(|err| err.to_string())?;
    std::fs::write(path, content).map_err(|err| err.to_string())
}
