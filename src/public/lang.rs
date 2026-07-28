use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const ASSETS_DIR_NAME: &str = "assets";
const LANG_DIR_NAME: &str = "lang";
const EMBEDDED_ICON_ASSETS: &[&str] = &["icon.ico", "icon.png"];

struct LangStore {
    zh: HashMap<String, String>,
    en: HashMap<String, String>,
    ja: HashMap<String, String>,
    es: HashMap<String, String>,
}

static LANG_STORE: OnceLock<LangStore> = OnceLock::new();

pub fn init_i18n(app_dir: &Path) {
    let lang_tables = load_language_tables(app_dir);
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

fn load_language_tables(app_dir: &Path) -> LangTables {
    LangTables {
        zh: load_language_toml(app_dir, "zh"),
        en: load_language_toml(app_dir, "en"),
        ja: load_language_toml(app_dir, "ja"),
        es: load_language_toml(app_dir, "es"),
    }
}

fn manifest_assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(ASSETS_DIR_NAME)
}

fn runtime_assets_dir(app_dir: &Path) -> PathBuf {
    app_dir.join(ASSETS_DIR_NAME)
}

fn asset_dir_candidates(app_dir: &Path) -> Vec<PathBuf> {
    let runtime_dir = runtime_assets_dir(app_dir);
    let manifest_dir = manifest_assets_dir();

    if runtime_dir == manifest_dir {
        vec![runtime_dir]
    } else {
        vec![runtime_dir, manifest_dir]
    }
}

fn is_embedded_icon_asset(segments: &[&str]) -> bool {
    matches!(segments, [file_name] if EMBEDDED_ICON_ASSETS.contains(file_name))
}

fn asset_path(app_dir: &Path, segments: &[&str]) -> Option<PathBuf> {
    if segments.is_empty() || is_embedded_icon_asset(segments) {
        return None;
    }

    asset_dir_candidates(app_dir)
        .into_iter()
        .map(|dir| {
            segments
                .iter()
                .fold(dir, |path, segment| path.join(segment))
        })
        .find(|path| path.is_file())
}

fn language_toml_path(app_dir: &Path, lang_code: &str) -> Option<PathBuf> {
    let file_name = format!("{lang_code}.toml");
    asset_path(app_dir, &[LANG_DIR_NAME, file_name.as_str()])
}

fn load_language_toml(app_dir: &Path, lang_code: &str) -> HashMap<String, String> {
    language_toml_path(app_dir, lang_code)
        .as_deref()
        .and_then(read_language_toml)
        .unwrap_or_default()
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
                    .join(ASSETS_DIR_NAME)
                    .join(LANG_DIR_NAME)
                    .join(format!("{lang_code}.toml")),
            );

            assert!(
                lang_map.is_some(),
                "assets/lang/{lang_code}.toml should be valid"
            );
        }
    }

    #[test]
    fn language_loader_reads_from_assets_dir() {
        let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        assert!(!load_language_toml(&project_dir, "zh").is_empty());
    }

    #[test]
    fn icon_assets_are_not_runtime_asset_files() {
        let project_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

        assert!(asset_path(project_dir, &["icon.ico"]).is_none());
        assert!(asset_path(project_dir, &["icon.png"]).is_none());
    }
}
