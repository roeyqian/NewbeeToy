use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_DIR_NAME: &str = "config";
const BASE_FILE_NAME: &str = "base.toml";
const LANG_FILE_NAME: &str = "lang.toml";
const ENV_FILE_NAME: &str = "env.toml";
const MIN_WINDOW_WIDTH: u32 = 540;
const MIN_WINDOW_HEIGHT: u32 = 320;
const DEFAULT_WINDOW_WIDTH: u32 = 1024;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguageConfig {
    pub english: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PathConfig {
    pub rename_folder: String,
    pub icon_source: String,
    pub icon_output: String,
    pub unlock_target: String,
    pub env_value_path: String,
    pub env_preset_path: String,
    pub env_variable_name: String,
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

impl Default for LanguageConfig {
    fn default() -> Self {
        Self { english: false }
    }
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            rename_folder: String::new(),
            icon_source: String::new(),
            icon_output: String::new(),
            unlock_target: String::new(),
            env_value_path: String::new(),
            env_preset_path: String::new(),
            env_variable_name: String::new(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            language: LanguageConfig::default(),
            paths: PathConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> bool {
        self.window.width >= MIN_WINDOW_WIDTH && self.window.height >= MIN_WINDOW_HEIGHT
    }
}

pub fn load_or_create_config(exe_dir: &Path) -> AppConfig {
    let _ = ensure_config_layout(exe_dir);
    let config_path = config_path(exe_dir);

    if let Ok(raw) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = toml::from_str::<AppConfig>(&raw) {
            if config.validate() {
                return config;
            }
        }
    }

    let config = AppConfig::default();
    let _ = save_config(exe_dir, &config);
    config
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

pub fn env_toml_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(ENV_FILE_NAME)
}

fn config_path(exe_dir: &Path) -> PathBuf {
    config_dir(exe_dir).join(BASE_FILE_NAME)
}

