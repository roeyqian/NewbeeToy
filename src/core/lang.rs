use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::config::lang_toml_path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LangToml {
    #[serde(default)]
    zh: HashMap<String, String>,
    #[serde(default)]
    en: HashMap<String, String>,
    #[serde(default)]
    ja: HashMap<String, String>,
    #[serde(default)]
    es: HashMap<String, String>,
}

impl LangToml {
    fn validate(&self) -> bool {
        [&self.zh, &self.en, &self.ja, &self.es]
            .into_iter()
            .all(|map| {
                !map.is_empty()
                    && map
                        .iter()
                        .all(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
            })
    }
}

struct LangStore {
    zh: HashMap<String, String>,
    en: HashMap<String, String>,
    ja: HashMap<String, String>,
    es: HashMap<String, String>,
}

static LANG_STORE: OnceLock<LangStore> = OnceLock::new();

pub fn init_i18n(exe_dir: &Path) {
    let lang_toml = load_or_create_lang_toml(exe_dir);
    let _ = LANG_STORE.set(LangStore {
        zh: lang_toml.zh,
        en: lang_toml.en,
        ja: lang_toml.ja,
        es: lang_toml.es,
    });
}

fn load_or_create_lang_toml(exe_dir: &Path) -> LangToml {
    let path = lang_toml_path(exe_dir);
    if let Some(mut lang_toml) = read_lang_toml(&path) {
        let defaults = load_asset_lang_toml(exe_dir);
        if defaults.validate() && merge_missing_language_keys(&mut lang_toml, defaults) {
            let _ = save_lang_toml(&path, &lang_toml);
        }
        return lang_toml;
    }

    let rebuilt = load_asset_lang_toml(exe_dir);

    let valid = if rebuilt.validate() {
        rebuilt
    } else {
        LangToml {
            zh: HashMap::new(),
            en: HashMap::new(),
            ja: HashMap::new(),
            es: HashMap::new(),
        }
    };

    let _ = save_lang_toml(&path, &valid);
    valid
}

fn load_asset_lang_toml(exe_dir: &Path) -> LangToml {
    LangToml {
        zh: load_language_map(exe_dir, "zh"),
        en: load_language_map(exe_dir, "en"),
        ja: load_language_map(exe_dir, "ja"),
        es: load_language_map(exe_dir, "es"),
    }
}

fn merge_missing_map_keys(
    target: &mut HashMap<String, String>,
    defaults: HashMap<String, String>,
) -> bool {
    let mut changed = false;
    for (key, value) in defaults {
        if !target.contains_key(&key) {
            target.insert(key, value);
            changed = true;
        }
    }
    changed
}

fn merge_missing_language_keys(target: &mut LangToml, defaults: LangToml) -> bool {
    let mut changed = false;
    changed |= merge_missing_map_keys(&mut target.zh, defaults.zh);
    changed |= merge_missing_map_keys(&mut target.en, defaults.en);
    changed |= merge_missing_map_keys(&mut target.ja, defaults.ja);
    changed |= merge_missing_map_keys(&mut target.es, defaults.es);
    changed
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
        if let Ok(content) = std::fs::read_to_string(path)
            && let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content)
            && !map.is_empty()
        {
            return map;
        }
    }

    HashMap::new()
}

fn store() -> &'static LangStore {
    LANG_STORE.get_or_init(|| LangStore {
        zh: HashMap::new(),
        en: HashMap::new(),
        ja: HashMap::new(),
        es: HashMap::new(),
    })
}

pub fn normalize_language_index(language_index: i32) -> i32 {
    match language_index {
        0..=3 => language_index,
        _ => 0,
    }
}

fn primary_language_map(store: &LangStore, language_index: i32) -> &HashMap<String, String> {
    match normalize_language_index(language_index) {
        1 => &store.en,
        2 => &store.ja,
        3 => &store.es,
        _ => &store.zh,
    }
}

pub fn t(language_index: i32, key: &str) -> String {
    let s = store();
    primary_language_map(s, language_index)
        .get(key)
        .cloned()
        .or_else(|| s.en.get(key).cloned())
        .or_else(|| s.zh.get(key).cloned())
        .unwrap_or_else(|| key.to_string())
}

pub fn tf(language_index: i32, key: &str, replacements: &[(&str, &str)]) -> String {
    let mut text = t(language_index, key);
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
