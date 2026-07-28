use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::config_dir;

const GENERAL_FILE_NAME: &str = "general.dat";
const FOLDERSTYLE_GROUP_LABELS: [&str; 10] =
    ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeneralDat {
    #[serde(default)]
    pub folderstyle: FolderStyleDat,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FolderStyleDat {
    #[serde(default)]
    pub groups: BTreeMap<String, FolderStyleGroupDat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FolderStyleGroupDat {
    #[serde(default)]
    pub folders: Vec<String>,
}

pub fn general_dat_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(GENERAL_FILE_NAME)
}

pub fn default_general_dat() -> GeneralDat {
    let mut data = GeneralDat::default();
    for label in FOLDERSTYLE_GROUP_LABELS {
        data.folderstyle
            .groups
            .entry(label.to_string())
            .or_insert_with(FolderStyleGroupDat::default);
    }
    data
}

pub fn normalize_general_dat(mut data: GeneralDat) -> GeneralDat {
    for label in FOLDERSTYLE_GROUP_LABELS {
        data.folderstyle
            .groups
            .entry(label.to_string())
            .or_insert_with(FolderStyleGroupDat::default);
    }
    data
}

pub fn read_general_dat_path(path: &Path) -> Result<GeneralDat, String> {
    if !path.exists() {
        return Ok(default_general_dat());
    }

    let content = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    toml::from_str::<GeneralDat>(&content)
        .map(normalize_general_dat)
        .map_err(|err| err.to_string())
}

pub fn write_general_dat_path(path: &Path, data: &GeneralDat) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let content = toml::to_string_pretty(data).map_err(|err| err.to_string())?;
    std::fs::write(path, content).map_err(|err| err.to_string())
}
