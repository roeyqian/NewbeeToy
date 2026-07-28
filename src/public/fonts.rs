use std::path::{Path, PathBuf};

const ASSETS_DIR_NAME: &str = "assets";
const FONT_DIR: &[&str] = &["fonts", "han-serif"];
const FONT_FILES: &[&str] = &[
    "SourceHanSerifCN-Light.otf",
    "SourceHanSerifCN-Medium.otf",
    "SourceHanSerifCN-Bold.otf",
];
const EMBEDDED_ICON_ASSETS: &[&str] = &["icon.ico", "icon.png"];

pub fn load_external_fonts(app_dir: &Path) {
    let font_paths = existing_font_paths(app_dir);
    if font_paths.is_empty() {
        return;
    }

    let mut collection = slint::fontique_08::shared_collection();
    collection.load_fonts_from_paths(font_paths);
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

fn asset_dir_path(app_dir: &Path, segments: &[&str]) -> Option<PathBuf> {
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
        .find(|path| path.is_dir())
}

fn existing_font_paths(app_dir: &Path) -> Vec<PathBuf> {
    let Some(font_dir) = asset_dir_path(app_dir, FONT_DIR) else {
        return Vec::new();
    };

    FONT_FILES
        .iter()
        .map(|file_name| font_dir.join(file_name))
        .filter(|path| path.is_file())
        .collect()
}
