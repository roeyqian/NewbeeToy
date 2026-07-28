use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const LANG_DIR_NAME: &str = "lang";

struct LangStore {
    zh: HashMap<String, String>,
    en: HashMap<String, String>,
    ja: HashMap<String, String>,
    es: HashMap<String, String>,
}

static LANG_STORE: OnceLock<LangStore> = OnceLock::new();

pub fn init_i18n(exe_dir: &Path) {
    let lang_tables = load_language_tables(exe_dir);
    let _ = LANG_STORE.set(LangStore {
        zh: lang_tables.zh,
        en: lang_tables.en,
        ja: lang_tables.ja,
        es: lang_tables.es,
    });
}

struct LangTables {
    zh: HashMap<String, String>,
    en: HashMap<String, String>,
    ja: HashMap<String, String>,
    es: HashMap<String, String>,
}

fn load_language_tables(exe_dir: &Path) -> LangTables {
    LangTables {
        zh: load_language_toml(exe_dir, "zh"),
        en: load_language_toml(exe_dir, "en"),
        ja: load_language_toml(exe_dir, "ja"),
        es: load_language_toml(exe_dir, "es"),
    }
}

fn lang_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join(LANG_DIR_NAME)
}

fn language_toml_path(exe_dir: &Path, lang_code: &str) -> PathBuf {
    lang_dir(exe_dir).join(format!("{}.toml", lang_code))
}

fn load_language_toml(exe_dir: &Path, lang_code: &str) -> HashMap<String, String> {
    read_language_toml(&language_toml_path(exe_dir, lang_code)).unwrap_or_default()
}

fn validate_language_map(map: &HashMap<String, String>) -> bool {
    !map.is_empty()
        && map
            .iter()
            .all(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
}

fn read_language_toml(path: &Path) -> Option<HashMap<String, String>> {
    let content = std::fs::read_to_string(path).ok()?;
    let lang_map = toml::from_str::<HashMap<String, String>>(&content).ok()?;
    validate_language_map(&lang_map).then_some(lang_map)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_toml_assets_are_readable() {
        let project_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

        for lang_code in ["zh", "en", "ja", "es"] {
            let lang_map = read_language_toml(
                &project_dir
                    .join("assets")
                    .join("lang")
                    .join(format!("{lang_code}.toml")),
            );

            assert!(
                lang_map.is_some(),
                "assets/lang/{lang_code}.toml should be valid"
            );
        }
    }

    #[test]
    fn language_loader_does_not_fallback_outside_runtime_lang_dir() {
        let missing_exe_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("missing-lang-test-dir");

        assert!(load_language_toml(&missing_exe_dir, "zh").is_empty());
    }
}
