use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const LANG_DIR_NAME: &str = "lang";
const LEGACY_LANG_FILE_NAME: &str = "lang.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LangTables {
    #[serde(default)]
    zh: HashMap<String, String>,
    #[serde(default)]
    en: HashMap<String, String>,
    #[serde(default)]
    ja: HashMap<String, String>,
    #[serde(default)]
    es: HashMap<String, String>,
}

impl LangTables {
    fn validate(&self) -> bool {
        [&self.zh, &self.en, &self.ja, &self.es]
            .into_iter()
            .all(|map| validate_language_map(map))
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
    let lang_tables = load_or_create_language_tables(exe_dir);
    let _ = LANG_STORE.set(LangStore {
        zh: lang_tables.zh,
        en: lang_tables.en,
        ja: lang_tables.ja,
        es: lang_tables.es,
    });
}

fn load_or_create_language_tables(exe_dir: &Path) -> LangTables {
    let legacy = read_legacy_lang_toml(exe_dir);
    LangTables {
        zh: load_or_create_language_toml(
            exe_dir,
            "zh",
            legacy.as_ref().map(|lang| lang.zh.clone()),
        ),
        en: load_or_create_language_toml(
            exe_dir,
            "en",
            legacy.as_ref().map(|lang| lang.en.clone()),
        ),
        ja: load_or_create_language_toml(
            exe_dir,
            "ja",
            legacy.as_ref().map(|lang| lang.ja.clone()),
        ),
        es: load_or_create_language_toml(
            exe_dir,
            "es",
            legacy.as_ref().map(|lang| lang.es.clone()),
        ),
    }
}

fn lang_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join(LANG_DIR_NAME)
}

fn language_toml_path(exe_dir: &Path, lang_code: &str) -> PathBuf {
    lang_dir(exe_dir).join(format!("{}.toml", lang_code))
}

fn legacy_lang_toml_path(exe_dir: &Path) -> PathBuf {
    exe_dir.join(LEGACY_LANG_FILE_NAME)
}

fn load_or_create_language_toml(
    exe_dir: &Path,
    lang_code: &str,
    legacy_map: Option<HashMap<String, String>>,
) -> HashMap<String, String> {
    let path = language_toml_path(exe_dir, lang_code);
    let defaults = load_asset_language_map(exe_dir, lang_code);
    let defaults_valid = validate_language_map(&defaults);

    let mut should_save = false;
    let existing = match read_language_toml(&path) {
        Some(map) => Some(map),
        None => {
            should_save = legacy_map.is_some();
            legacy_map
        }
    };

    if let Some(mut lang_map) = existing {
        if defaults_valid && merge_missing_map_keys(&mut lang_map, defaults) {
            should_save = true;
        }
        if should_save {
            let _ = save_language_toml(&path, &lang_map);
        }
        return lang_map;
    }

    let rebuilt = if defaults_valid {
        defaults
    } else {
        HashMap::new()
    };

    let _ = save_language_toml(&path, &rebuilt);
    rebuilt
}

fn validate_language_map(map: &HashMap<String, String>) -> bool {
    !map.is_empty()
        && map
            .iter()
            .all(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
}

fn read_legacy_lang_toml(exe_dir: &Path) -> Option<LangTables> {
    let path = legacy_lang_toml_path(exe_dir);
    let content = std::fs::read_to_string(path).ok()?;
    let lang_tables = toml::from_str::<LangTables>(&content).ok()?;
    lang_tables.validate().then_some(lang_tables)
}

fn read_language_toml(path: &Path) -> Option<HashMap<String, String>> {
    let content = std::fs::read_to_string(path).ok()?;
    let lang_map = toml::from_str::<BTreeMap<String, String>>(&content).ok()?;
    let lang_map = lang_map.into_iter().collect::<HashMap<_, _>>();
    validate_language_map(&lang_map).then_some(lang_map)
}

fn save_language_toml(path: &Path, lang_map: &HashMap<String, String>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let sorted = lang_map
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let content = toml::to_string_pretty(&sorted).map_err(std::io::Error::other)?;
    std::fs::write(path, content)
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

fn load_asset_language_map(exe_dir: &Path, lang_code: &str) -> HashMap<String, String> {
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
