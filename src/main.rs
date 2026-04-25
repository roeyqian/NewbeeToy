#![windows_subsystem = "windows"]

mod core;

use core::config::{env_toml_path, load_or_create_config, save_config};
use core::general::rename::setup_rename_handlers;
use core::general::unlock::setup_unlock_handlers;
use core::lang::{init_i18n, sanitize_ui_text, t};
use core::media::icon::setup_icon_handlers;
use core::system::env::setup_env_handlers;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnableMenuItem, GWL_STYLE, GetSystemMenu, GetWindowLongPtrW, MF_BYCOMMAND, MF_DISABLED,
    MF_ENABLED, MF_GRAYED, SC_MAXIMIZE, SC_MOVE, SC_SIZE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_MAXIMIZEBOX,
    WS_THICKFRAME,
};

const HELP_URL: &str = "https://github.com/roeyqian/NewbeeToy";

slint::include_modules!();

fn open_help_url(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
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
            0, 0,
            0, 0,
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
                apply_window_lock(&ui.window(), ui.get_lock_window());
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

    None
}

fn append_log_line(current: &str, message: &str) -> String {
    let mut lines = if current.trim().is_empty() {
        Vec::new()
    } else {
        current.lines().map(|s| s.to_string()).collect::<Vec<_>>()
    };

    lines.push(format!("[INFO] {}", sanitize_ui_text(message)));

    const MAX_LOG_LINES: usize = 100;
    if lines.len() > MAX_LOG_LINES {
        let drop_count = lines.len() - MAX_LOG_LINES;
        lines.drain(0..drop_count);
    }

    lines.join("\n")
}

fn append_feature_ready_log(ui: &MainWindow, page: i32) {
    match page {
        1 => {
            let next = append_log_line(
                &ui.get_status_text().to_string(),
                &t(ui.get_lang_en(), "rename.msg.ready"),
            );
            ui.set_status_text(next.into());
        }
        2 => {
            let next = append_log_line(
                &ui.get_icon_status_text().to_string(),
                &t(ui.get_lang_en(), "icon.msg.ready"),
            );
            ui.set_icon_status_text(next.into());
        }
        3 => {
            let next = append_log_line(
                &ui.get_unlock_status_text().to_string(),
                &t(ui.get_lang_en(), "unlock.msg.ready"),
            );
            ui.set_unlock_status_text(next.into());
        }
        4 => {
            let next = append_log_line(
                &ui.get_env_status_text().to_string(),
                &t(ui.get_lang_en(), "env.msg.ready"),
            );
            ui.set_env_status_text(next.into());
        }
        _ => {}
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let app_dir = resolve_app_dir();
    init_i18n(&app_dir);
    let app_config = load_or_create_config(&app_dir);

    let ui = MainWindow::new()?;
    ui.set_lang_en(app_config.language.english);
    ui.set_lock_window(app_config.window.lock_window);

    if app_config.window.fullscreen {
        ui.window().set_fullscreen(true);
    } else {
        ui.window().set_size(slint::LogicalSize::new(
            app_config.window.width as f32,
            app_config.window.height as f32,
        ));
        ui.window().set_position(slint::LogicalPosition::new(
            app_config.window.x as f32,
            app_config.window.y as f32,
        ));
    }

    let default_dir = std::env::current_dir()
        .ok()
        .map(|dir| sanitize_ui_text(&dir.to_string_lossy()))
        .unwrap_or_default();

    let rename_folder = if app_config.paths.rename_folder.trim().is_empty() {
        default_dir.clone()
    } else {
        app_config.paths.rename_folder.clone()
    };
    ui.set_folder_path(rename_folder.into());

    let icon_source = if app_config.paths.icon_source.trim().is_empty() {
        default_dir.clone()
    } else {
        app_config.paths.icon_source.clone()
    };
    ui.set_icon_source_path(icon_source.into());

    let icon_output = if app_config.paths.icon_output.trim().is_empty() {
        default_dir.clone()
    } else {
        app_config.paths.icon_output.clone()
    };
    ui.set_icon_output_path(icon_output.into());

    let env_value_path = if app_config.paths.env_value_path.trim().is_empty() {
        default_dir.clone()
    } else {
        app_config.paths.env_value_path.clone()
    };
    ui.set_env_value_path(env_value_path.into());

    let env_preset_path = if app_config.paths.env_preset_path.trim().is_empty() {
        sanitize_ui_text(&env_toml_path(&app_dir).to_string_lossy())
    } else {
        app_config.paths.env_preset_path.clone()
    };
    ui.set_env_preset_path(env_preset_path.into());

    ui.set_env_variable_name(app_config.paths.env_variable_name.clone().into());

    ui.set_unlock_target_path(app_config.paths.unlock_target.clone().into());

    ui.on_pick_folder(|start_path| {
        let mut dialog = rfd::FileDialog::new();
        if let Some(dir) = resolve_dialog_start_dir(start_path.as_str()) {
            dialog = dialog.set_directory(dir);
        }

        dialog
            .pick_folder()
            .map(|path| sanitize_ui_text(&path.to_string_lossy()))
            .unwrap_or_default()
            .into()
    });

    ui.on_pick_icon_file(|start_path| {
        let mut dialog = rfd::FileDialog::new();
        if let Some(dir) = resolve_dialog_start_dir(start_path.as_str()) {
            dialog = dialog.set_directory(dir);
        }

        dialog
            .add_filter("Icon Sources", &["exe", "dll", "icl", "lnk", "ico"])
            .pick_file()
            .map(|path| sanitize_ui_text(&path.to_string_lossy()))
            .unwrap_or_default()
            .into()
    });

    ui.on_pick_unlock_file(|start_path| {
        let mut dialog = rfd::FileDialog::new();
        if let Some(dir) = resolve_dialog_start_dir(start_path.as_str()) {
            dialog = dialog.set_directory(dir);
        }

        dialog
            .pick_file()
            .map(|path| sanitize_ui_text(&path.to_string_lossy()))
            .unwrap_or_default()
            .into()
    });

    ui.on_tr(|key, lang_en| t(lang_en, key.as_str()).into());

    ui.on_set_language_request({
        let ui_handle = ui.as_weak();
        move |lang_en| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_lang_en(lang_en);
            }
        }
    });

    ui.on_set_window_lock_request({
        let ui_handle = ui.as_weak();
        move |lock_window| {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_lock_window(lock_window);
                apply_window_lock(&ui.window(), lock_window);
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
                ui.set_env_status_text("".into());
            }
        }
    });

    ui.on_open_help_request(|| {
        open_help_url(HELP_URL);
    });

    let rename_inited = Rc::new(RefCell::new(false));
    let icon_inited = Rc::new(RefCell::new(false));
    let unlock_inited = Rc::new(RefCell::new(false));
    let env_inited = Rc::new(RefCell::new(false));

    ui.on_open_feature_request({
        let ui_handle = ui.as_weak();
        let app_dir = app_dir.clone();
        let rename_inited = Rc::clone(&rename_inited);
        let icon_inited = Rc::clone(&icon_inited);
        let unlock_inited = Rc::clone(&unlock_inited);
        let env_inited = Rc::clone(&env_inited);
        move |page| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            match page {
                1 => {
                    let was_inited = *rename_inited.borrow();
                    if !was_inited {
                        setup_rename_handlers(&ui);
                        *rename_inited.borrow_mut() = true;
                    } else {
                        append_feature_ready_log(&ui, 1);
                    }
                }
                2 => {
                    let was_inited = *icon_inited.borrow();
                    if !was_inited {
                        setup_icon_handlers(&ui);
                        *icon_inited.borrow_mut() = true;
                    } else {
                        append_feature_ready_log(&ui, 2);
                    }
                }
                3 => {
                    let was_inited = *unlock_inited.borrow();
                    if !was_inited {
                        setup_unlock_handlers(&ui);
                        *unlock_inited.borrow_mut() = true;
                    } else {
                        append_feature_ready_log(&ui, 3);
                    }
                }
                4 => {
                    let was_inited = *env_inited.borrow();
                    if !was_inited {
                        setup_env_handlers(&ui, &app_dir);
                        *env_inited.borrow_mut() = true;
                    } else {
                        append_feature_ready_log(&ui, 4);
                    }
                    ui.invoke_env_enter_request();
                }
                _ => {}
            }

            ui.set_page_index(page);
        }
    });

    apply_window_lock(&ui.window(), app_config.window.lock_window);
    schedule_window_lock_reapply(&ui);

    let run_result = ui.run();

    let mut final_config = load_or_create_config(&app_dir);
    final_config.language.english = ui.get_lang_en();
    final_config.window.fullscreen = ui.window().is_fullscreen();
    final_config.window.lock_window = ui.get_lock_window();
    final_config.paths.rename_folder = ui.get_folder_path().to_string();
    final_config.paths.icon_source = ui.get_icon_source_path().to_string();
    final_config.paths.icon_output = ui.get_icon_output_path().to_string();
    final_config.paths.unlock_target = ui.get_unlock_target_path().to_string();
    final_config.paths.env_value_path = ui.get_env_value_path().to_string();
    final_config.paths.env_preset_path = ui.get_env_preset_path().to_string();
    final_config.paths.env_variable_name = ui.get_env_variable_name().to_string();

    if !final_config.window.fullscreen {
        let current_size = ui.window().size().to_logical(ui.window().scale_factor());
        final_config.window.width = current_size.width.max(0.0) as u32;
        final_config.window.height = current_size.height.max(0.0) as u32;

        if !final_config.window.lock_window {
            let current_position = ui
                .window()
                .position()
                .to_logical(ui.window().scale_factor());
            final_config.window.x = current_position.x.round() as i32;
            final_config.window.y = current_position.y.round() as i32;
        }
    }

    let _ = save_config(&app_dir, &final_config);
    run_result
}

