fn main() {
    write_default_runtime_config();
    copy_runtime_assets();

    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().unwrap();
    }
    slint_build::compile("gui/main.slint").unwrap();
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
    write_if_missing(&config_dir.join("general.dat"), default_general_dat());
    write_if_missing(&config_dir.join("system.dat"), default_system_dat());
}

fn target_profile_dir() -> Option<std::path::PathBuf> {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR")?);
    out_dir
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .map(std::path::Path::to_path_buf)
}

fn write_if_missing(path: &std::path::Path, content: &str) {
    if path.exists() {
        return;
    }

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

fn default_general_dat() -> &'static str {
    r#"[folderstyle.groups.I]
folders = []

[folderstyle.groups.II]
folders = []

[folderstyle.groups.III]
folders = []

[folderstyle.groups.IV]
folders = []

[folderstyle.groups.V]
folders = []

[folderstyle.groups.VI]
folders = []

[folderstyle.groups.VII]
folders = []

[folderstyle.groups.VIII]
folders = []

[folderstyle.groups.IX]
folders = []

[folderstyle.groups.X]
folders = []
"#
}

fn default_system_dat() -> &'static str {
    "[presets]\n"
}
