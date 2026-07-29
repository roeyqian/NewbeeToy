use std::path::{Path, PathBuf};

use super::config_dir;

const BASE_FILE_NAME: &str = "base.toml";
pub use super::schema::AppConfig;

pub fn load_or_create_config(exe_dir: &Path) -> AppConfig {
    load_or_create_config_with_save_error(exe_dir).0
}

pub fn load_or_create_config_with_save_error(
    exe_dir: &Path,
) -> (AppConfig, Option<std::io::Error>) {
    let layout_error = ensure_base_config_layout(exe_dir).err();
    let config_path = base_toml_path(exe_dir);

    if let Ok(config) = read_config_path(&config_path)
        && config.validate()
    {
        return (config, layout_error);
    }

    let config = AppConfig::default();
    let save_error = save_config(exe_dir, &config).err().or(layout_error);
    (config, save_error)
}

pub fn save_config(exe_dir: &Path, config: &AppConfig) -> std::io::Result<()> {
    ensure_base_config_layout(exe_dir)?;
    let config_path = base_toml_path(exe_dir);
    let content = toml::to_string_pretty(config).map_err(std::io::Error::other)?;
    std::fs::write(config_path, content)
}

pub fn ensure_base_config_layout(exe_dir: &Path) -> std::io::Result<()> {
    let dir = config_dir(exe_dir);
    std::fs::create_dir_all(&dir)?;

    let base_path = base_toml_path(exe_dir);
    if !base_path.exists() {
        save_config(exe_dir, &AppConfig::default())?;
    }

    Ok(())
}

pub fn base_toml_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(BASE_FILE_NAME)
}

fn read_config_path(path: &Path) -> Result<AppConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    toml::from_str::<AppConfig>(&raw).map_err(|err| err.to_string())
}
