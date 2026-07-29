use std::path::{Path, PathBuf};

use super::{
    config_dir,
    format::{decode_binary_dat, write_binary_dat_path},
    schema,
};

const SYSTEM_FILE_NAME: &str = "system.dat";
pub use schema::{SystemDat, SystemPresetDat};

pub fn system_dat_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(SYSTEM_FILE_NAME)
}

pub fn read_system_dat_path(path: &Path) -> Result<SystemDat, String> {
    if !path.exists() {
        return Ok(SystemDat::default());
    }

    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
    decode_binary_dat::<SystemDat>(&bytes).or_else(|_| read_legacy_toml_system_dat(&bytes))
}

pub fn write_system_dat_path(path: &Path, data: &SystemDat) -> Result<(), String> {
    write_binary_dat_path(path, data)
}

fn read_legacy_toml_system_dat(bytes: &[u8]) -> Result<SystemDat, String> {
    let content = std::str::from_utf8(bytes).map_err(|err| err.to_string())?;
    toml::from_str::<SystemDat>(content).map_err(|err| err.to_string())
}
