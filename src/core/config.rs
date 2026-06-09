use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_DIR_NAME: &str = "config";
const BASE_FILE_NAME: &str = "base.toml";
const LANG_FILE_NAME: &str = "lang.toml";
const SYSENV_FILE_NAME: &str = "sysenv.toml";
const MIN_WINDOW_WIDTH: u32 = 540;
const MIN_WINDOW_HEIGHT: u32 = 320;
const DEFAULT_WINDOW_WIDTH: u32 = 1024;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub language: LanguageConfig,
    #[serde(default)]
    pub paths: PathConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub fullscreen: bool,
    pub lock_window: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LanguageConfig {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PathConfig {
    pub rename_folder: String,
    pub icon_source: String,
    pub icon_output: String,
    pub unlock_target: String,
    #[serde(alias = "env_value_path")]
    pub sysenv_value_path: String,
    #[serde(alias = "env_preset_path")]
    pub sysenv_preset_path: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WINDOW_WIDTH,
            height: DEFAULT_WINDOW_HEIGHT,
            x: 80,
            y: 80,
            fullscreen: false,
            lock_window: false,
        }
    }
}

impl LanguageConfig {
    pub fn language_index(&self) -> i32 {
        match self.code.trim() {
            "zh" => 0,
            "en" => 1,
            "ja" => 2,
            "es" => 3,
            _ => 0,
        }
    }

    pub fn set_language_index(&mut self, language_index: i32) {
        self.code = match language_index {
            1 => "en",
            2 => "ja",
            3 => "es",
            _ => "zh",
        }
        .to_string();
    }
}

impl AppConfig {
    pub fn validate(&self) -> bool {
        self.window.width >= MIN_WINDOW_WIDTH && self.window.height >= MIN_WINDOW_HEIGHT
    }
}

pub fn load_or_create_config(exe_dir: &Path) -> AppConfig {
    load_or_create_config_with_save_error(exe_dir).0
}

pub fn load_or_create_config_with_save_error(
    exe_dir: &Path,
) -> (AppConfig, Option<std::io::Error>) {
    let layout_error = ensure_config_layout(exe_dir).err();
    let config_path = config_path(exe_dir);

    if let Ok(raw) = std::fs::read_to_string(&config_path)
        && let Ok(config) = toml::from_str::<AppConfig>(&raw)
        && config.validate()
    {
        return (config, layout_error);
    }

    let config = AppConfig::default();
    let save_error = save_config(exe_dir, &config).err().or(layout_error);
    (config, save_error)
}

pub fn save_config(exe_dir: &Path, config: &AppConfig) -> std::io::Result<()> {
    ensure_config_layout(exe_dir)?;
    let config_path = config_path(exe_dir);
    let content = toml::to_string_pretty(config).map_err(std::io::Error::other)?;
    std::fs::write(config_path, content)
}

pub fn ensure_config_layout(exe_dir: &Path) -> std::io::Result<()> {
    let dir = config_dir(exe_dir);
    std::fs::create_dir_all(&dir)?;

    let base_path = config_path(exe_dir);
    if !base_path.exists() {
        let content =
            toml::to_string_pretty(&AppConfig::default()).map_err(std::io::Error::other)?;
        std::fs::write(base_path, content)?;
    }

    Ok(())
}

pub fn config_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join(CONFIG_DIR_NAME)
}

pub fn lang_toml_path(exe_dir: &Path) -> PathBuf {
    exe_dir.join(LANG_FILE_NAME)
}

pub fn sysenv_toml_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(SYSENV_FILE_NAME)
}

pub fn base_toml_path(exe_dir: &Path) -> PathBuf {
    config_path(exe_dir)
}

fn config_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(BASE_FILE_NAME)
}
