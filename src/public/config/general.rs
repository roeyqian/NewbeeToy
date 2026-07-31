use std::path::{Path, PathBuf};

use super::{
    config_dir,
    format::{decode_binary_dat, write_binary_dat_path},
    schema,
};

const GENERAL_FILE_NAME: &str = "general.dat";
pub use schema::{FolderStylePresetDat, GeneralDat};

pub fn general_dat_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(GENERAL_FILE_NAME)
}

pub fn default_general_dat() -> GeneralDat {
    schema::default_general_dat()
}

pub fn normalize_general_dat(data: GeneralDat) -> GeneralDat {
    schema::normalize_general_dat(data)
}

pub fn read_general_dat_path(path: &Path) -> Result<GeneralDat, String> {
    if !path.exists() {
        return Ok(default_general_dat());
    }

    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;

    decode_binary_dat::<GeneralDat>(&bytes)
        .or_else(|_| read_legacy_toml_general_dat(&bytes))
        .map(normalize_general_dat)
}

pub fn write_general_dat_path(path: &Path, data: &GeneralDat) -> Result<(), String> {
    write_binary_dat_path(path, data)
}

fn read_legacy_toml_general_dat(bytes: &[u8]) -> Result<GeneralDat, String> {
    let content = std::str::from_utf8(bytes).map_err(|err| err.to_string())?;
    toml::from_str::<GeneralDat>(content).map_err(|err| err.to_string())
}
