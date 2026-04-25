use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::core::config::env_toml_path;
use crate::core::lang::{sanitize_ui_text, t, tf};
use crate::{EnvPreviewRow, MainWindow};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EnvToml {
    #[serde(default)]
    value_path: String,
    #[serde(default)]
    variable_name: String,
    #[serde(default)]
    variables: BTreeMap<String, String>,
}

fn append_env_status_log(ui: &MainWindow, level: &str, message: &str) {
    let current = ui.get_env_status_text().to_string();
    let mut lines: Vec<String> = if current.trim().is_empty() {
        Vec::new()
    } else {
        current.lines().map(|s| s.to_string()).collect()
    };

    lines.push(format!("[{}] {}", level, sanitize_ui_text(message)));

    const MAX_LOG_LINES: usize = 100;
    if lines.len() > MAX_LOG_LINES {
        let drop_count = lines.len() - MAX_LOG_LINES;
        lines.drain(0..drop_count);
    }

    ui.set_env_status_text(lines.join("\n").into());
}

fn set_preview_rows(ui: &MainWindow, vars: &BTreeMap<String, String>) {
    let mapped = vars
        .iter()
        .map(|(name, value)| EnvPreviewRow {
            name: sanitize_ui_text(name).into(),
            value: sanitize_ui_text(value).into(),
        })
        .collect::<Vec<_>>();
    ui.set_env_preview_rows(ModelRc::new(VecModel::from(mapped)));
}

fn apply_vars_to_ui(ui: &MainWindow, vars: &BTreeMap<String, String>) {
    if vars.is_empty() {
        ui.set_env_preview_text(t(ui.get_lang_en(), "env.msg.empty_system_env").into());
        ui.set_env_preview_rows(ModelRc::new(VecModel::from(Vec::<EnvPreviewRow>::new())));
    } else {
        ui.set_env_preview_text("".into());
        set_preview_rows(ui, vars);
    }
}

fn reload_system_env_to_preview(
    ui: &MainWindow,
    preview_state: &Rc<RefCell<BTreeMap<String, String>>>,
    removed_names: &Rc<RefCell<HashSet<String>>>,
) {
    match read_system_env_variables() {
        Ok(vars) => {
            apply_vars_to_ui(ui, &vars);
            *preview_state.borrow_mut() = vars;
            removed_names.borrow_mut().clear();
            append_env_status_log(ui, "INFO", &t(ui.get_lang_en(), "env.msg.system_loaded"));
        }
        Err(err) => {
            append_env_status_log(
                ui,
                "ERROR",
                &tf(
                    ui.get_lang_en(),
                    "env.msg.system_load_failed",
                    &[("error", &err)],
                ),
            );
        }
    }
}

fn reset_apply_progress(ui: &MainWindow, apply_armed: &Rc<RefCell<bool>>) {
    let mut armed = apply_armed.borrow_mut();
    if *armed {
        *armed = false;
        append_env_status_log(
            ui,
            "INFO",
            &t(ui.get_lang_en(), "env.msg.apply_progress_reset"),
        );
    }
}

fn read_env_toml(path: &Path) -> Result<EnvToml, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    toml::from_str::<EnvToml>(&content).map_err(|e| e.to_string())
}

fn save_env_toml(path: &Path, env_toml: &EnvToml) -> Result<(), String> {
    let content = toml::to_string_pretty(env_toml).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

fn resolve_preset_path(raw_path: &str, fallback_path: &Path) -> PathBuf {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return fallback_path.to_path_buf();
    }

    PathBuf::from(trimmed)
}

fn sync_broadcast_env_changed() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };

    let env_text = "Environment\0".encode_utf16().collect::<Vec<u16>>();
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env_text.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            std::ptr::null_mut(),
        );
    }
}

fn reg_utf16_bytes_to_string(bytes: &[u8]) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }

    while units.last().copied() == Some(0) {
        units.pop();
    }

    sanitize_ui_text(&String::from_utf16_lossy(&units))
}

fn read_system_env_variables() -> Result<BTreeMap<String, String>, String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_MULTI_SZ, REG_SZ};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env_key = hklm
        .open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            KEY_READ,
        )
        .map_err(|e| e.to_string())?;

    let mut vars = BTreeMap::new();
    for item in env_key.enum_values() {
        let Ok((name, value)) = item else {
            continue;
        };

        let parsed = match value.vtype {
            REG_SZ | REG_EXPAND_SZ => reg_utf16_bytes_to_string(&value.bytes),
            REG_MULTI_SZ => reg_utf16_bytes_to_string(&value.bytes).replace('\u{0}', ";"),
            _ => continue,
        };

        vars.insert(name, parsed);
    }

    Ok(vars)
}

fn write_system_env_variable(name: &str, value: &str) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_SET_VALUE};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env_key = hklm
        .open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            KEY_SET_VALUE,
        )
        .map_err(|e| e.to_string())?;

    env_key.set_value(name, &value).map_err(|e| e.to_string())?;
    sync_broadcast_env_changed();
    Ok(())
}

fn delete_system_env_variable(name: &str) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_SET_VALUE};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env_key = hklm
        .open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            KEY_SET_VALUE,
        )
        .map_err(|e| e.to_string())?;

    match env_key.delete_value(name) {
        Ok(()) => {
            sync_broadcast_env_changed();
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

pub fn setup_env_handlers(ui: &MainWindow, app_dir: &Path) {
    let env_path = env_toml_path(app_dir);
    let preview_state: Rc<RefCell<BTreeMap<String, String>>> =
        Rc::new(RefCell::new(BTreeMap::new()));
    let removed_names: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
    let apply_armed: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    ui.set_env_status_text("".into());
    ui.set_env_preview_text("".into());
    ui.set_env_preview_rows(ModelRc::new(VecModel::from(Vec::<EnvPreviewRow>::new())));
    append_env_status_log(ui, "INFO", &t(ui.get_lang_en(), "env.msg.ready"));

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let removed_names = Rc::clone(&removed_names);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_env_enter_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);
            reload_system_env_to_preview(&ui, &preview_state, &removed_names);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_env_interaction_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let apply_armed = Rc::clone(&apply_armed);
        let env_path = env_path.clone();
        ui.on_env_store_request(move |preset_path, value_path, variable_name| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);

            let preset_path = resolve_preset_path(preset_path.as_str(), &env_path);
            let data = EnvToml {
                value_path: value_path.as_str().trim().to_string(),
                variable_name: variable_name.as_str().trim().to_string(),
                variables: preview_state.borrow().clone(),
            };

            if let Some(parent) = preset_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            match save_env_toml(&preset_path, &data) {
                Ok(()) => {
                    let path_text = preset_path.display().to_string();
                    ui.set_env_preset_path(path_text.clone().into());
                    append_env_status_log(
                        &ui,
                        "INFO",
                        &tf(
                            ui.get_lang_en(),
                            "env.msg.store_success",
                            &[("path", &path_text)],
                        ),
                    );
                }
                Err(err) => {
                    append_env_status_log(
                        &ui,
                        "ERROR",
                        &tf(ui.get_lang_en(), "env.msg.store_failed", &[("error", &err)]),
                    );
                }
            }
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let removed_names = Rc::clone(&removed_names);
        let apply_armed = Rc::clone(&apply_armed);
        let env_path = env_path.clone();
        ui.on_env_load_preset_request(move |preset_path| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);

            let preset_path = resolve_preset_path(preset_path.as_str(), &env_path);
            let path_text = preset_path.display().to_string();

            match read_env_toml(&preset_path) {
                Ok(data) => {
                    ui.set_env_preset_path(path_text.clone().into());
                    ui.set_env_value_path(data.value_path.into());
                    ui.set_env_variable_name(data.variable_name.into());
                    apply_vars_to_ui(&ui, &data.variables);
                    *preview_state.borrow_mut() = data.variables;
                    removed_names.borrow_mut().clear();
                    append_env_status_log(
                        &ui,
                        "INFO",
                        &tf(
                            ui.get_lang_en(),
                            "env.msg.load_success",
                            &[("path", &path_text)],
                        ),
                    );
                }
                Err(err) => {
                    let err_with_path = format!("{} ({})", err, path_text);
                    append_env_status_log(
                        &ui,
                        "ERROR",
                        &tf(
                            ui.get_lang_en(),
                            "env.msg.load_failed",
                            &[("error", &err_with_path)],
                        ),
                    );
                }
            }
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let removed_names = Rc::clone(&removed_names);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_env_load_system_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);
            reload_system_env_to_preview(&ui, &preview_state, &removed_names);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let removed_names = Rc::clone(&removed_names);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_env_preview_request(move |value_path, variable_name| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);

            let value = value_path.as_str().trim().to_string();
            if value.is_empty() {
                append_env_status_log(&ui, "ERROR", &t(ui.get_lang_en(), "env.msg.value_required"));
                return;
            }

            let name = variable_name.as_str().trim().to_string();
            if name.is_empty() {
                append_env_status_log(
                    &ui,
                    "ERROR",
                    &t(ui.get_lang_en(), "env.msg.variable_name_required"),
                );
                return;
            }

            let mut updated = preview_state.borrow().clone();
            updated.insert(name.clone(), value.clone());
            removed_names.borrow_mut().remove(&name);
            apply_vars_to_ui(&ui, &updated);
            *preview_state.borrow_mut() = updated;

            append_env_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_lang_en(),
                    "env.msg.preview_success",
                    &[("name", &name), ("value", &value)],
                ),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let removed_names = Rc::clone(&removed_names);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_env_remove_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);

            let mut vars = preview_state.borrow().clone();
            let row_index = index as usize;
            let Some((name, _)) = vars
                .iter()
                .nth(row_index)
                .map(|(k, v)| (k.clone(), v.clone()))
            else {
                return;
            };

            vars.remove(&name);
            removed_names.borrow_mut().insert(name.clone());
            apply_vars_to_ui(&ui, &vars);
            *preview_state.borrow_mut() = vars;

            append_env_status_log(
                &ui,
                "INFO",
                &tf(ui.get_lang_en(), "env.msg.row_removed", &[("name", &name)]),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let removed_names = Rc::clone(&removed_names);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_env_commit_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let snapshot = preview_state.borrow().clone();

            if !*apply_armed.borrow() {
                let system_vars = match read_system_env_variables() {
                    Ok(vars) => vars,
                    Err(err) => {
                        append_env_status_log(
                            &ui,
                            "ERROR",
                            &tf(
                                ui.get_lang_en(),
                                "env.msg.system_load_failed",
                                &[("error", &err)],
                            ),
                        );
                        return;
                    }
                };

                let mut add_count = 0usize;
                let mut change_count = 0usize;
                for (name, value) in &snapshot {
                    match system_vars.get(name) {
                        None => add_count += 1,
                        Some(old) if old != value => change_count += 1,
                        _ => {}
                    }
                }

                let delete_count = removed_names
                    .borrow()
                    .iter()
                    .filter(|name| system_vars.contains_key(*name))
                    .count();

                if snapshot.is_empty() && delete_count == 0 {
                    append_env_status_log(
                        &ui,
                        "ERROR",
                        &t(ui.get_lang_en(), "env.msg.preview_empty"),
                    );
                    *apply_armed.borrow_mut() = false;
                    return;
                }

                if add_count == 0 && change_count == 0 && delete_count == 0 {
                    append_env_status_log(
                        &ui,
                        "INFO",
                        &t(ui.get_lang_en(), "env.msg.apply_no_changes"),
                    );
                    *apply_armed.borrow_mut() = false;
                    return;
                }

                *apply_armed.borrow_mut() = true;
                append_env_status_log(
                    &ui,
                    "WARN",
                    &tf(
                        ui.get_lang_en(),
                        "env.msg.apply_confirm_pending",
                        &[
                            ("add", &add_count.to_string()),
                            ("change", &change_count.to_string()),
                            ("delete", &delete_count.to_string()),
                        ],
                    ),
                );
                return;
            }

            *apply_armed.borrow_mut() = false;
            append_env_status_log(
                &ui,
                "INFO",
                &t(ui.get_lang_en(), "env.msg.apply_confirm_execute"),
            );

            let system_vars = match read_system_env_variables() {
                Ok(vars) => vars,
                Err(err) => {
                    append_env_status_log(
                        &ui,
                        "ERROR",
                        &tf(
                            ui.get_lang_en(),
                            "env.msg.system_load_failed",
                            &[("error", &err)],
                        ),
                    );
                    return;
                }
            };

            let targets = snapshot
                .iter()
                .filter_map(|(name, value)| {
                    (system_vars.get(name) != Some(value)).then_some((name.clone(), value.clone()))
                })
                .collect::<Vec<_>>();

            let delete_targets = removed_names
                .borrow()
                .iter()
                .filter(|name| system_vars.contains_key(*name))
                .cloned()
                .collect::<Vec<_>>();

            if targets.is_empty() && delete_targets.is_empty() {
                append_env_status_log(
                    &ui,
                    "INFO",
                    &t(ui.get_lang_en(), "env.msg.apply_no_changes"),
                );
                return;
            }

            let mut ok_count = 0usize;
            let mut fail_count = 0usize;
            for (name, value) in &targets {
                match write_system_env_variable(name, value) {
                    Ok(()) => ok_count += 1,
                    Err(err) => {
                        fail_count += 1;
                        append_env_status_log(
                            &ui,
                            "ERROR",
                            &tf(
                                ui.get_lang_en(),
                                "env.msg.apply_item_failed",
                                &[("name", name), ("error", &err)],
                            ),
                        );
                    }
                }
            }

            for name in &delete_targets {
                match delete_system_env_variable(name) {
                    Ok(()) => ok_count += 1,
                    Err(err) => {
                        fail_count += 1;
                        append_env_status_log(
                            &ui,
                            "ERROR",
                            &tf(
                                ui.get_lang_en(),
                                "env.msg.apply_delete_failed",
                                &[("name", name), ("error", &err)],
                            ),
                        );
                    }
                }
            }

            removed_names.borrow_mut().clear();

            append_env_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_lang_en(),
                    "env.msg.apply_preview_done",
                    &[
                        ("ok", &ok_count.to_string()),
                        ("failed", &fail_count.to_string()),
                    ],
                ),
            );
        });
    }
}

