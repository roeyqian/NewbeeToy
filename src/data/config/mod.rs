use std::path::{Path, PathBuf};

pub mod base;
pub mod general;
pub mod system;

pub use base::{
    AppConfig, base_toml_path, load_or_create_config, load_or_create_config_with_save_error,
    save_config,
};
pub use general::general_dat_path;
pub use system::system_dat_path;

const CONFIG_DIR_NAME: &str = "config";

pub fn config_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join(CONFIG_DIR_NAME)
}
