use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::config::lang_toml_path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LangToml {
    zh: HashMap<String, String>,
    en: HashMap<String, String>,
}

impl LangToml {
    fn validate(&self) -> bool {
        !self.zh.is_empty()
            && !self.en.is_empty()
            && self
                .zh
                .iter()
                .all(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
            && self
                .en
                .iter()
                .all(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
    }
}

struct LangStore {
    zh: HashMap<String, String>,
    en: HashMap<String, String>,
}

static LANG_STORE: OnceLock<LangStore> = OnceLock::new();

pub fn init_i18n(exe_dir: &Path) {
    let lang_toml = load_or_create_lang_toml(exe_dir);
    let _ = LANG_STORE.set(LangStore {
        zh: lang_toml.zh,
        en: lang_toml.en,
    });
}

fn load_or_create_lang_toml(exe_dir: &Path) -> LangToml {
    let path = lang_toml_path(exe_dir);
    if let Some(lang_toml) = read_lang_toml(&path) {
        return lang_toml;
    }

    let rebuilt = LangToml {
        zh: load_language_map(exe_dir, "zh"),
        en: load_language_map(exe_dir, "en"),
    };

    let valid = if rebuilt.validate() {
        rebuilt
    } else {
        LangToml {
            zh: HashMap::new(),
            en: HashMap::new(),
        }
    };

    let _ = save_lang_toml(&path, &valid);
    valid
}

fn read_lang_toml(path: &Path) -> Option<LangToml> {
    let content = std::fs::read_to_string(path).ok()?;
    let lang_toml = toml::from_str::<LangToml>(&content).ok()?;
    lang_toml.validate().then_some(lang_toml)
}

fn save_lang_toml(path: &Path, lang_toml: &LangToml) -> std::io::Result<()> {
    let content = toml::to_string_pretty(lang_toml).map_err(std::io::Error::other)?;
    std::fs::write(path, content)
}

fn load_language_map(exe_dir: &Path, lang_code: &str) -> HashMap<String, String> {
    let mut candidate_paths = vec![
        exe_dir.join("lang").join(format!("{}.json", lang_code)),
        exe_dir
            .join("assets")
            .join("lang")
            .join(format!("{}.json", lang_code)),
    ];

    candidate_paths.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("lang")
            .join(format!("{}.json", lang_code)),
    );

    for path in candidate_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
                if !map.is_empty() {
                    return map;
                }
            }
        }
    }

    HashMap::new()
}

fn store() -> &'static LangStore {
    LANG_STORE.get_or_init(|| LangStore {
        zh: HashMap::new(),
        en: HashMap::new(),
    })
}

pub fn t(lang_en: bool, key: &str) -> String {
    let s = store();
    if lang_en {
        s.en.get(key)
            .cloned()
            .or_else(|| s.zh.get(key).cloned())
            .unwrap_or_else(|| key.to_string())
    } else {
        s.zh.get(key)
            .cloned()
            .or_else(|| s.en.get(key).cloned())
            .unwrap_or_else(|| key.to_string())
    }
}

pub fn tf(lang_en: bool, key: &str, replacements: &[(&str, &str)]) -> String {
    let mut text = t(lang_en, key);
    for (name, value) in replacements {
        let placeholder = format!("{{{}}}", name);
        text = text.replace(&placeholder, value);
    }
    text
}

pub fn sanitize_ui_text(input: &str) -> String {
    input
        .chars()
        .filter_map(|ch| match ch {
            '\u{fffd}' => Some('?'),
            '\r' => None,
            '\n' | '\t' => Some(ch),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

