use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MIN_WINDOW_WIDTH: u32 = 540;
const MIN_WINDOW_HEIGHT: u32 = 320;
const DEFAULT_WINDOW_WIDTH: u32 = 1024;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;
const FOLDERSTYLE_GROUP_LABELS: [&str; 10] =
    ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];

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
    pub sysenv_preset_name: String,
    pub folderstyle_folder: String,
}

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
