#![windows_subsystem = "windows"]

mod core;
mod public;

use core::general::folderstyle::setup_folderstyle_handlers;
use core::general::rename::setup_rename_handlers;
use core::general::unlock::setup_unlock_handlers;
use core::media::icon::setup_icon_handlers;
use core::system::sysenv::setup_sysenv_handlers;
use core::util::append_log_line;
use public::assets::fonts::load_external_fonts;
use public::assets::lang::{init_i18n, normalize_language_index, sanitize_ui_text, t};
use public::config::{
    AppConfig, base_toml_path, load_or_create_config, load_or_create_config_with_save_error,
    save_config,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnableMenuItem, GWL_STYLE, GetSystemMenu, GetWindowLongPtrW, MF_BYCOMMAND, MF_DISABLED,
    MF_ENABLED, MF_GRAYED, SC_MAXIMIZE, SC_MOVE, SC_SIZE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_MAXIMIZEBOX,
    WS_THICKFRAME,
};

const HELP_URL: &str = "https://github.com/roeyqian/NewbeeToy";
const DEFAULT_SYSENV_PRESET_NAME: &str = "default";
const DEFAULT_FOLDERSTYLE_PRESET_NAME: &str = "default";
const APP_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

slint::include_modules!();

#[derive(Copy, Clone, Eq, PartialEq)]
enum FeaturePage {
    Rename,
    Icon,
    Unlock,
    Sysenv,
    FolderStyle,
}

impl FeaturePage {
    fn from_index(page: i32) -> Option<Self> {
        match page {
            1 => Some(Self::Rename),
            2 => Some(Self::Icon),
            3 => Some(Self::Unlock),
            4 => Some(Self::Sysenv),
            5 => Some(Self::FolderStyle),
            _ => None,
        }
    }
}

#[derive(Default)]
struct FeatureInitState {
    rename: bool,
    icon: bool,
    unlock: bool,
    sysenv: bool,
    folderstyle: bool,
}

impl FeatureInitState {
    fn take_first_visit(&mut self, page: FeaturePage) -> bool {
        let initialized = match page {
            FeaturePage::Rename => &mut self.rename,
            FeaturePage::Icon => &mut self.icon,
            FeaturePage::Unlock => &mut self.unlock,
            FeaturePage::Sysenv => &mut self.sysenv,
            FeaturePage::FolderStyle => &mut self.folderstyle,
        };
        let was_first_visit = !*initialized;
        *initialized = true;
        was_first_visit
    }
}

fn open_help_url(url: &str) {
    let _ = opener::open(url);
}

fn show_base_config_save_error(app_dir: &Path, error: &std::io::Error) {
    let path = base_toml_path(app_dir);
    let description = format!(
        "Unable to save configuration to:\n{}\n\n{}\n\nSettings changed in this session may not be preserved. Move NewbeeToy to a writable folder or run it with sufficient permissions.",
        path.display(),
        error
    );

    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("NewbeeToy configuration save failed")
        .set_description(description)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

fn resolve_app_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn apply_window_lock(window: &slint::Window, locked: bool) {
    let slint_window_handle = window.window_handle();
    let Ok(handle) = slint_window_handle.window_handle() else {
        return;
    };

    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };

    let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    if hwnd.is_null() {
        return;
    }

    unsafe {
        let mut style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if locked {
            style &= !(WS_THICKFRAME | WS_MAXIMIZEBOX);
        } else {
            style |= WS_THICKFRAME | WS_MAXIMIZEBOX;
        }
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);

        let system_menu = GetSystemMenu(hwnd, 0);
        if !system_menu.is_null() {
            let menu_state = if locked {
                MF_BYCOMMAND | MF_DISABLED | MF_GRAYED
            } else {
                MF_BYCOMMAND | MF_ENABLED
            };
            EnableMenuItem(system_menu, SC_MOVE, menu_state);
            EnableMenuItem(system_menu, SC_SIZE, menu_state);
            EnableMenuItem(system_menu, SC_MAXIMIZE, menu_state);
        }

        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

fn schedule_window_lock_reapply(ui: &MainWindow) {
    let ui_handle = ui.as_weak();
    for delay_ms in [0_u64, 60, 240] {
        let ui_handle = ui_handle.clone();
        slint::Timer::single_shot(Duration::from_millis(delay_ms), move || {
            if let Some(ui) = ui_handle.upgrade() {
                apply_window_lock(ui.window(), ui.get_lock_window());
            }
        });
    }
}

fn resolve_dialog_start_dir(input: &str) -> Option<PathBuf> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = PathBuf::from(trimmed);
    if path.is_dir() {
        return Some(path);
    }

    if path.is_file() {
        return path.parent().map(|p| p.to_path_buf());
    }

    if let Some(parent) = path.parent()
        && parent.is_dir()
    {
        return Some(parent.to_path_buf());
    }

    None
}

fn dialog_with_start_dir(start_path: &str) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();
    if let Some(dir) = resolve_dialog_start_dir(start_path) {
        dialog = dialog.set_directory(dir);
    }
    dialog
}

fn selected_dialog_path(path: Option<PathBuf>) -> slint::SharedString {
    path.map(|path| sanitize_ui_text(&path.to_string_lossy()))
        .unwrap_or_default()
        .into()
}

fn pick_folder_path(start_path: &str) -> slint::SharedString {
    selected_dialog_path(dialog_with_start_dir(start_path).pick_folder())
}

fn pick_file_path(start_path: &str) -> slint::SharedString {
    selected_dialog_path(dialog_with_start_dir(start_path).pick_file())
}

fn pick_icon_file_path(start_path: &str) -> slint::SharedString {
    selected_dialog_path(
        dialog_with_start_dir(start_path)
            .add_filter("Icon Sources", &["exe", "dll", "icl", "ico"])
            .pick_file(),
    )
}

fn append_feature_ready_log(ui: &MainWindow, page: i32) {
    match page {
        1 => {
            let next = append_log_line(
                ui.get_status_text().as_ref(),
                &t(ui.get_language_index(), "rename.msg.ready"),
            );
            ui.set_status_text(next.into());
        }
        2 => {
            let next = append_log_line(
                ui.get_icon_status_text().as_ref(),
                &t(ui.get_language_index(), "icon.msg.ready"),
            );
            ui.set_icon_status_text(next.into());
        }
        3 => {
            let next = append_log_line(
                ui.get_unlock_status_text().as_ref(),
                &t(ui.get_language_index(), "unlock.msg.ready"),
            );
            ui.set_unlock_status_text(next.into());
        }
        4 => {
            let next = append_log_line(
                ui.get_sysenv_status_text().as_ref(),
                &t(ui.get_language_index(), "sysenv.msg.ready"),
            );
            ui.set_sysenv_status_text(next.into());
        }
        5 => {
            let next = append_log_line(
                ui.get_folderstyle_status_text().as_ref(),
                &t(ui.get_language_index(), "folderstyle.msg.ready"),
            );
            ui.set_folderstyle_status_text(next.into());
        }
        _ => {}
    }
}

fn apply_window_config(ui: &MainWindow, config: &AppConfig) {
    if config.window.fullscreen {
        ui.window().set_fullscreen(true);
    } else {
        ui.window().set_size(slint::LogicalSize::new(
            config.window.width as f32,
            config.window.height as f32,
        ));
        ui.window().set_position(slint::LogicalPosition::new(
            config.window.x as f32,
            config.window.y as f32,
        ));
    }

    ui.set_lock_window(config.window.lock_window);
}

fn apply_path_defaults(ui: &MainWindow, config: &AppConfig) {
    let default_dir = std::env::current_dir()
        .ok()
        .map(|dir| sanitize_ui_text(&dir.to_string_lossy()))
        .unwrap_or_default();

    let rename_folder = if config.paths.rename_folder.trim().is_empty() {
        default_dir.clone()
    } else {
        config.paths.rename_folder.clone()
    };
    ui.set_folder_path(rename_folder.into());

    let icon_source = if config.paths.icon_source.trim().is_empty() {
        default_dir.clone()
    } else {
        config.paths.icon_source.clone()
    };
    ui.set_icon_source_path(icon_source.into());

    let icon_output = if config.paths.icon_output.trim().is_empty() {
        default_dir.clone()
    } else {
        config.paths.icon_output.clone()
    };
    ui.set_icon_output_path(icon_output.into());

    let sysenv_value_path = if config.paths.sysenv_value_path.trim().is_empty() {
        default_dir.clone()
    } else {
        config.paths.sysenv_value_path.clone()
    };
    ui.set_sysenv_value_path(sysenv_value_path.into());

    let sysenv_preset_name = if config.paths.sysenv_preset_name.trim().is_empty() {
        DEFAULT_SYSENV_PRESET_NAME.to_string()
    } else {
        config.paths.sysenv_preset_name.clone()
    };
    ui.set_sysenv_preset_name(sysenv_preset_name.into());

    let folderstyle_folder = if config.paths.folderstyle_folder.trim().is_empty() {
        default_dir.clone()
    } else {
        config.paths.folderstyle_folder.clone()
    };
    ui.set_folderstyle_folder_path(folderstyle_folder.into());

    let folderstyle_preset_name = if config.paths.folderstyle_preset_name.trim().is_empty() {
        DEFAULT_FOLDERSTYLE_PRESET_NAME.to_string()
    } else {
        config.paths.folderstyle_preset_name.clone()
    };
    ui.set_folderstyle_preset_name(folderstyle_preset_name.into());

    ui.set_unlock_target_path(config.paths.unlock_target.clone().into());
}

fn collect_runtime_config(ui: &MainWindow, app_dir: &Path) -> AppConfig {
    let mut config = load_or_create_config(app_dir);
    config
        .language
        .set_language_index(normalize_language_index(ui.get_language_index()));
    config.window.fullscreen = ui.window().is_fullscreen();
    config.window.lock_window = ui.get_lock_window();
    config.paths.rename_folder = ui.get_folder_path().to_string();
    config.paths.icon_source = ui.get_icon_source_path().to_string();
    config.paths.icon_output = ui.get_icon_output_path().to_string();
    config.paths.unlock_target = ui.get_unlock_target_path().to_string();
    config.paths.sysenv_value_path = ui.get_sysenv_value_path().to_string();
    config.paths.sysenv_preset_name = ui.get_sysenv_preset_name().to_string();
    config.paths.folderstyle_folder = ui.get_folderstyle_folder_path().to_string();
    config.paths.folderstyle_preset_name = ui.get_folderstyle_preset_name().to_string();

    if !config.window.fullscreen {
        let current_size = ui.window().size().to_logical(ui.window().scale_factor());
        config.window.width = current_size.width.max(0.0) as u32;
        config.window.height = current_size.height.max(0.0) as u32;

        if !config.window.lock_window {
            let current_position = ui
                .window()
                .position()
                .to_logical(ui.window().scale_factor());
            config.window.x = current_position.x.round() as i32;
            config.window.y = current_position.y.round() as i32;
        }
    }

    config
}

fn setup_feature_page(ui: &MainWindow, app_dir: &Path, page: FeaturePage, first_visit: bool) {
    match page {
        FeaturePage::Rename => {
            if first_visit {
                setup_rename_handlers(ui);
            } else {
                append_feature_ready_log(ui, 1);
            }
        }
        FeaturePage::Icon => {
            if first_visit {
                setup_icon_handlers(ui);
            } else {
                append_feature_ready_log(ui, 2);
            }
        }
        FeaturePage::Unlock => {
            if first_visit {
                setup_unlock_handlers(ui);
            } else {
                append_feature_ready_log(ui, 3);
            }
        }
        FeaturePage::Sysenv => {
            if first_visit {
                setup_sysenv_handlers(ui, app_dir);
            } else {
                append_feature_ready_log(ui, 4);
            }
            ui.invoke_sysenv_enter_request();
        }
        FeaturePage::FolderStyle => {
            if first_visit {
                setup_folderstyle_handlers(ui, app_dir);
            } else {
                append_feature_ready_log(ui, 5);
            }
        }
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let app_dir = resolve_app_dir();
    init_i18n(&app_dir);
    let (app_config, startup_config_save_error) = load_or_create_config_with_save_error(&app_dir);
    if let Some(error) = startup_config_save_error {
        show_base_config_save_error(&app_dir, &error);
    }

    let ui = MainWindow::new()?;
    load_external_fonts(&app_dir);
    ui.set_app_version(APP_VERSION.into());
    ui.set_language_index(app_config.language.language_index());
    apply_window_config(&ui, &app_config);
    apply_path_defaults(&ui, &app_config);

    ui.on_pick_folder(|start_path| pick_folder_path(start_path.as_str()));

    ui.on_pick_icon_file(|start_path| pick_icon_file_path(start_path.as_str()));

    ui.on_pick_unlock_file(|start_path| pick_file_path(start_path.as_str()));

    ui.on_pick_unlock_folder(|start_path| pick_folder_path(start_path.as_str()));

    ui.on_tr(|key, language_index| t(language_index, key.as_str()).into());

    ui.on_set_language_request({
        let ui_handle = ui.as_weak();
        move |language_index| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_language_index(normalize_language_index(language_index));
            }
        }
    });

    ui.on_set_window_lock_request({
        let ui_handle = ui.as_weak();
        move |lock_window| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_lock_window(lock_window);
                apply_window_lock(ui.window(), lock_window);
                schedule_window_lock_reapply(&ui);
            }
        }
    });

    ui.on_clear_logs_request({
        let ui_handle = ui.as_weak();
        move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_status_text("".into());
                ui.set_icon_status_text("".into());
                ui.set_unlock_status_text("".into());
                ui.set_sysenv_status_text("".into());
                ui.set_folderstyle_status_text("".into());
            }
        }
    });

    ui.on_open_help_request(|| {
        open_help_url(HELP_URL);
    });

    let feature_state = Rc::new(RefCell::new(FeatureInitState::default()));

    ui.on_open_feature_request({
        let ui_handle = ui.as_weak();
        let app_dir = app_dir.clone();
        let feature_state = Rc::clone(&feature_state);
        move |page| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            if let Some(feature_page) = FeaturePage::from_index(page) {
                let first_visit = feature_state.borrow_mut().take_first_visit(feature_page);
                setup_feature_page(&ui, &app_dir, feature_page, first_visit);
            }

            ui.set_page_index(page);
        }
    });

    apply_window_lock(ui.window(), app_config.window.lock_window);
    schedule_window_lock_reapply(&ui);

    let run_result = ui.run();

    let final_config = collect_runtime_config(&ui, &app_dir);
    if let Err(error) = save_config(&app_dir, &final_config) {
        show_base_config_save_error(&app_dir, &error);
    }
    run_result
}
