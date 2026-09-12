#[allow(dead_code)]
mod config_schema {
    include!("src/data/schema.rs");
}

fn main() {
    emit_rerun_instructions();
    write_default_runtime_config();
    copy_runtime_assets();

    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().unwrap();
    }
    slint_build::compile("gui/main.slint").unwrap();
}

fn emit_rerun_instructions() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/data/schema.rs");
    println!("cargo:rerun-if-changed=assets/lang");
    println!("cargo:rerun-if-changed=assets/fonts");
}

fn copy_runtime_assets() {
    let Some(profile_dir) = target_profile_dir() else {
        return;
    };

    let runtime_assets_dir = profile_dir.join("assets");
    copy_dir_contents(
        std::path::Path::new("assets").join("lang").as_path(),
        runtime_assets_dir.join("lang").as_path(),
    );
    copy_dir_contents(
        std::path::Path::new("assets").join("fonts").as_path(),
        runtime_assets_dir.join("fonts").as_path(),
    );
}

fn write_default_runtime_config() {
    let Some(profile_dir) = target_profile_dir() else {
        return;
    };

    let config_dir = profile_dir.join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    write_toml_if_missing(
        &config_dir.join("general.toml"),
        &config_schema::default_general_config(),
    );
    write_toml_if_missing(
        &config_dir.join("system.toml"),
        &config_schema::SystemConfig::default(),
    );
}

fn target_profile_dir() -> Option<std::path::PathBuf> {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR")?);
    out_dir
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .map(std::path::Path::to_path_buf)
}

fn write_toml_if_missing<T: serde::Serialize>(path: &std::path::Path, default_data: &T) {
    if path.exists() {
        return;
    }

    let content = toml::to_string_pretty(default_data).unwrap();
    std::fs::write(path, content).unwrap();
}

fn copy_dir_contents(source: &std::path::Path, destination: &std::path::Path) {
    println!("cargo:rerun-if-changed={}", source.display());
    std::fs::create_dir_all(destination).unwrap();

    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_contents(&source_path, &destination_path);
        } else if source_path.is_file() {
            println!("cargo:rerun-if-changed={}", source_path.display());
            std::fs::copy(&source_path, &destination_path).unwrap();
        }
    }
}
