use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::core::util::append_log_line;
use crate::public::assets::lang::{sanitize_ui_text, t, tf};
use crate::public::config::{
    system::{SystemPresetDat, read_system_dat_path, write_system_dat_path},
    system_dat_path,
};
use crate::{MainWindow, SysenvPreviewRow, SysenvValueEditorWindow, SysenvValueEntry};

#[derive(Default)]
struct SysenvPreviewState {
    variables: BTreeMap<String, String>,
}

impl SysenvPreviewState {
    fn clear(&mut self) {
        self.variables.clear();
    }

    fn snapshot(&self) -> BTreeMap<String, String> {
        self.variables.clone()
    }

    fn replace(&mut self, variables: BTreeMap<String, String>) {
        self.variables = variables;
    }

    fn merge(&mut self, variables: BTreeMap<String, String>) {
        self.variables.extend(variables);
    }

    fn insert_new(
        &mut self,
        name: String,
        value: String,
        language_index: i32,
    ) -> Result<(), String> {
        validate_sysenv_variable_name(&name, language_index)?;
        if self.variables.contains_key(&name) {
            return Err(tf(
                language_index,
                "sysenv.msg.variable_already_exists",
                &[("name", &name)],
            ));
        }

        self.variables.insert(name, value);
        Ok(())
    }

    fn update_existing(&mut self, name: &str, value: String) -> bool {
        let Some(existing) = self.variables.get_mut(name) else {
            return false;
        };
        *existing = value;
        true
    }

    fn remove_row(&mut self, row_index: usize) -> Option<String> {
        let name = self.variables.keys().nth(row_index).cloned()?;
        self.variables.remove(&name);
        Some(name)
    }

    fn row(&self, row_index: usize) -> Option<(String, String)> {
        self.variables
            .iter()
            .nth(row_index)
            .map(|(name, value)| (name.clone(), value.clone()))
    }
}

fn append_sysenv_status_log(ui: &MainWindow, _level: &str, message: &str) {
    ui.set_sysenv_status_text(
        append_log_line(ui.get_sysenv_status_text().as_ref(), message).into(),
    );
}

fn append_sysenv_system_load_failed(ui: &MainWindow, err: &str) {
    append_sysenv_status_log(
        ui,
        "ERROR",
        &tf(
            ui.get_language_index(),
            "sysenv.msg.system_load_failed",
            &[("error", err)],
        ),
    );
}

fn sysenv_result_or_log<T>(ui: &MainWindow, result: Result<T, String>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(err) => {
            append_sysenv_system_load_failed(ui, &err);
            None
        }
    }
}

fn set_preview_rows(ui: &MainWindow, vars: &BTreeMap<String, String>) {
    ui.set_sysenv_preview_rows(ModelRc::new(VecModel::from(preview_rows_from_vars(vars))));
}

fn preview_rows_from_vars(vars: &BTreeMap<String, String>) -> Vec<SysenvPreviewRow> {
    vars.iter()
        .map(|(name, value)| SysenvPreviewRow {
            name: sanitize_ui_text(name).into(),
            value: sanitize_ui_text(value).into(),
        })
        .collect::<Vec<_>>()
}

fn value_entries_from_env_value(value: &str) -> Vec<SysenvValueEntry> {
    if value.is_empty() {
        return vec![SysenvValueEntry { value: "".into() }];
    }

    value
        .split(';')
        .map(|entry| SysenvValueEntry {
            value: sanitize_ui_text(entry).into(),
        })
        .collect()
}

fn set_value_editor_entries(editor: &SysenvValueEditorWindow, entries: &[String]) {
    let rows = entries
        .iter()
        .map(|entry| SysenvValueEntry {
            value: sanitize_ui_text(entry).into(),
        })
        .collect::<Vec<_>>();
    editor.set_entries(ModelRc::new(VecModel::from(rows)));
}

fn apply_vars_to_ui(ui: &MainWindow, vars: &BTreeMap<String, String>) {
    ui.set_sysenv_preview_text("".into());
    set_preview_rows(ui, vars);
}

fn reload_system_env_to_preview(ui: &MainWindow, preview_state: &Rc<RefCell<SysenvPreviewState>>) {
    match read_system_env_variables() {
        Ok(vars) => {
            apply_vars_to_ui(ui, &vars);
            preview_state.borrow_mut().replace(vars);
            append_sysenv_status_log(
                ui,
                "INFO",
                &t(ui.get_language_index(), "sysenv.msg.system_loaded"),
            );
        }
        Err(err) => {
            append_sysenv_system_load_failed(ui, &err);
        }
    }
}

fn reset_apply_progress(ui: &MainWindow, apply_armed: &Rc<RefCell<bool>>) {
    let mut armed = apply_armed.borrow_mut();
    if *armed {
        *armed = false;
        append_sysenv_status_log(
            ui,
            "INFO",
            &t(ui.get_language_index(), "sysenv.msg.apply_progress_reset"),
        );
    }
}

fn normalize_preset_name(raw_name: &str, language_index: i32) -> Result<String, String> {
    let name = sanitize_ui_text(raw_name.trim());
    if name.is_empty() {
        Err(t(language_index, "sysenv.msg.preset_name_required"))
    } else {
        Ok(name)
    }
}

fn store_sysenv_preset(
    path: &Path,
    preset_name: &str,
    variables: BTreeMap<String, String>,
) -> Result<(), String> {
    let mut data = read_system_dat_path(path)?;
    data.presets
        .insert(preset_name.to_string(), SystemPresetDat { variables });
    write_system_dat_path(path, &data)
}

fn load_sysenv_preset(
    path: &Path,
    preset_name: &str,
    language_index: i32,
) -> Result<SystemPresetDat, String> {
    let data = read_system_dat_path(path)?;
    data.presets.get(preset_name).cloned().ok_or_else(|| {
        tf(
            language_index,
            "sysenv.msg.preset_not_found",
            &[("name", preset_name)],
        )
    })
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

fn pick_sysenv_value_entry_path(start_path: &str) -> Option<String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(dir) = resolve_dialog_start_dir(start_path) {
        dialog = dialog.set_directory(dir);
    }

    dialog
        .pick_folder()
        .map(|path| sanitize_ui_text(&path.to_string_lossy()))
}

fn apply_value_editor_window_config(editor: &SysenvValueEditorWindow, ui: &MainWindow) {
    let scale_factor = ui.window().scale_factor();
    let parent_size = ui.window().size().to_logical(scale_factor);
    let parent_position = ui.window().position().to_logical(scale_factor);
    let parent_width = parent_size.width;
    let parent_height = parent_size.height;
    let editor_width = parent_width * 3.0 / 4.0;
    let editor_height = parent_height * 3.0 / 4.0;
    let editor_x = parent_position.x + (parent_width - editor_width) / 2.0;
    let editor_y = parent_position.y + (parent_height - editor_height) / 2.0;

    editor
        .window()
        .set_size(slint::LogicalSize::new(editor_width, editor_height));
    editor
        .window()
        .set_position(slint::LogicalPosition::new(editor_x, editor_y));
}

fn schedule_delayed_value_editor_window_config_apply(
    editor: &SysenvValueEditorWindow,
    ui: &MainWindow,
    delay: Duration,
) {
    let editor_handle = editor.as_weak();
    let ui_handle = ui.as_weak();
    slint::Timer::single_shot(delay, move || {
        if let (Some(editor), Some(ui)) = (editor_handle.upgrade(), ui_handle.upgrade()) {
            apply_value_editor_window_config(&editor, &ui);
        }
    });
}

fn schedule_value_editor_window_config_apply(editor: &SysenvValueEditorWindow, ui: &MainWindow) {
    apply_value_editor_window_config(editor, ui);
    schedule_delayed_value_editor_window_config_apply(editor, ui, Duration::from_millis(0));
    schedule_delayed_value_editor_window_config_apply(editor, ui, Duration::from_millis(60));
}

fn validate_sysenv_variable_name(name: &str, language_index: i32) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err(t(language_index, "sysenv.msg.variable_name_required"));
    }

    if name.contains('=') {
        return Err(t(language_index, "sysenv.msg.variable_name_invalid"));
    }

    Ok(())
}

fn show_sysenv_value_editor(
    ui: &MainWindow,
    preview_state: &Rc<RefCell<SysenvPreviewState>>,
    apply_armed: &Rc<RefCell<bool>>,
    index: i32,
) {
    reset_apply_progress(ui, apply_armed);

    let row_index = index as usize;
    let vars = preview_state.borrow();
    let Some((name, value)) = vars.row(row_index) else {
        return;
    };
    drop(vars);

    let editor = match SysenvValueEditorWindow::new() {
        Ok(editor) => editor,
        Err(err) => {
            append_sysenv_system_load_failed(ui, &err.to_string());
            return;
        }
    };

    let entries = value_entries_from_env_value(&value)
        .into_iter()
        .map(|entry| entry.value.to_string())
        .collect::<Vec<_>>();
    let entry_state = Rc::new(RefCell::new(entries));

    editor.set_variable_name(sanitize_ui_text(&name).into());
    editor.set_language_index(ui.get_language_index());
    set_value_editor_entries(&editor, &entry_state.borrow());
    editor.on_tr(|key, language_index| t(language_index, key.as_str()).into());

    let editor_lifetime: Rc<RefCell<Option<SysenvValueEditorWindow>>> = Rc::new(RefCell::new(None));

    {
        let editor_handle = editor.as_weak();
        let entry_state = Rc::clone(&entry_state);
        editor.on_update_entry_request(move |entry_index, value| {
            if editor_handle.upgrade().is_none() {
                return;
            }

            let index = entry_index as usize;
            let mut entries = entry_state.borrow_mut();
            if let Some(entry) = entries.get_mut(index) {
                *entry = sanitize_ui_text(value.as_str());
            }
        });
    }

    {
        let editor_handle = editor.as_weak();
        let entry_state = Rc::clone(&entry_state);
        editor.on_add_entry_request(move || {
            let Some(editor) = editor_handle.upgrade() else {
                return;
            };

            let mut entries = entry_state.borrow_mut();
            entries.push(String::new());
            set_value_editor_entries(&editor, &entries);
        });
    }

    {
        let editor_handle = editor.as_weak();
        let entry_state = Rc::clone(&entry_state);
        editor.on_browse_entry_request(move |entry_index| {
            let Some(editor) = editor_handle.upgrade() else {
                return;
            };

            let index = entry_index as usize;
            let current = entry_state.borrow().get(index).cloned().unwrap_or_default();
            let Some(selected) = pick_sysenv_value_entry_path(&current) else {
                return;
            };

            let mut entries = entry_state.borrow_mut();
            if let Some(entry) = entries.get_mut(index) {
                *entry = selected;
                set_value_editor_entries(&editor, &entries);
            }
        });
    }

    {
        let editor_handle = editor.as_weak();
        let entry_state = Rc::clone(&entry_state);
        editor.on_remove_entry_request(move |entry_index| {
            let Some(editor) = editor_handle.upgrade() else {
                return;
            };

            let index = entry_index as usize;
            let mut entries = entry_state.borrow_mut();
            if index < entries.len() {
                entries.remove(index);
                if entries.is_empty() {
                    entries.push(String::new());
                }
                set_value_editor_entries(&editor, &entries);
            }
        });
    }

    {
        let editor_handle = editor.as_weak();
        let entry_state = Rc::clone(&entry_state);
        editor.on_move_entry_request(move |from, to| {
            let Some(editor) = editor_handle.upgrade() else {
                return;
            };

            let from = from as usize;
            let to = to as usize;
            let mut entries = entry_state.borrow_mut();
            if from < entries.len() && to < entries.len() {
                entries.swap(from, to);
                set_value_editor_entries(&editor, &entries);
            }
        });
    }

    {
        let editor_handle = editor.as_weak();
        let editor_lifetime = Rc::clone(&editor_lifetime);
        editor.on_cancel_request(move || {
            if let Some(editor) = editor_handle.upgrade() {
                let _ = editor.hide();
            }
            editor_lifetime.borrow_mut().take();
        });
    }

    {
        let ui_handle = ui.as_weak();
        let editor_handle = editor.as_weak();
        let editor_lifetime = Rc::clone(&editor_lifetime);
        let preview_state = Rc::clone(preview_state);
        let entry_state = Rc::clone(&entry_state);
        editor.on_accept_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let value = entry_state.borrow().join(";");
            let mut state = preview_state.borrow_mut();
            if !state.update_existing(&name, sanitize_ui_text(&value)) {
                if let Some(editor) = editor_handle.upgrade() {
                    let _ = editor.hide();
                }
                editor_lifetime.borrow_mut().take();
                return;
            }

            let snapshot = state.snapshot();
            drop(state);
            apply_vars_to_ui(&ui, &snapshot);

            append_sysenv_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "sysenv.msg.row_edited",
                    &[("name", &name)],
                ),
            );

            if let Some(editor) = editor_handle.upgrade() {
                let _ = editor.hide();
            }
            editor_lifetime.borrow_mut().take();
        });
    }

    let _ = editor.show();
    schedule_value_editor_window_config_apply(&editor, ui);
    *editor_lifetime.borrow_mut() = Some(editor);
}

fn validate_sysenv_variable_snapshot(
    vars: &BTreeMap<String, String>,
    language_index: i32,
) -> Result<(), String> {
    for name in vars.keys() {
        validate_sysenv_variable_name(name, language_index)?;
    }

    Ok(())
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

fn registry_string_bytes(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn contains_env_reference(value: &str) -> bool {
    let chars = value.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] != '%' {
            index += 1;
            continue;
        }

        let start = index + 1;
        let Some(end) = chars[start..]
            .iter()
            .position(|ch| *ch == '%')
            .map(|offset| start + offset)
        else {
            return false;
        };
        if start < end
            && chars[start..end]
                .iter()
                .all(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '(' || *ch == ')')
        {
            return true;
        }

        index += 1;
    }

    false
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

fn read_system_env_variable_types() -> Result<BTreeMap<String, winreg::enums::RegType>, String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_MULTI_SZ, REG_SZ};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env_key = hklm
        .open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            KEY_READ,
        )
        .map_err(|e| e.to_string())?;

    let mut types = BTreeMap::new();
    for item in env_key.enum_values() {
        let Ok((name, value)) = item else {
            continue;
        };

        if matches!(value.vtype, REG_SZ | REG_EXPAND_SZ | REG_MULTI_SZ) {
            types.insert(name, value.vtype);
        }
    }

    Ok(types)
}

fn write_system_env_variable(
    name: &str,
    value: &str,
    existing_type: Option<&winreg::enums::RegType>,
) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::RegValue;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env_key = hklm
        .open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            KEY_SET_VALUE,
        )
        .map_err(|e| e.to_string())?;

    let vtype = if existing_type == Some(&REG_EXPAND_SZ) || contains_env_reference(value) {
        REG_EXPAND_SZ
    } else {
        REG_SZ
    };
    let raw = RegValue {
        bytes: Cow::Owned(registry_string_bytes(value)),
        vtype,
    };

    env_key
        .set_raw_value(name, &raw)
        .map_err(|e| e.to_string())?;
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

fn reset_sysenv_panel(
    ui: &MainWindow,
    preview_state: &Rc<RefCell<SysenvPreviewState>>,
    apply_armed: &Rc<RefCell<bool>>,
) {
    preview_state.borrow_mut().clear();
    *apply_armed.borrow_mut() = false;

    ui.set_sysenv_preview_text("".into());
    ui.set_sysenv_preview_rows(ModelRc::new(VecModel::from(Vec::<SysenvPreviewRow>::new())));
}

pub fn setup_sysenv_handlers(ui: &MainWindow, app_dir: &Path) {
    let system_path = system_dat_path(app_dir);
    let preview_state: Rc<RefCell<SysenvPreviewState>> =
        Rc::new(RefCell::new(SysenvPreviewState::default()));
    let apply_armed: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    reset_sysenv_panel(ui, &preview_state, &apply_armed);
    ui.set_sysenv_status_text("".into());
    append_sysenv_status_log(ui, "INFO", &t(ui.get_language_index(), "sysenv.msg.ready"));

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_sysenv_enter_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_sysenv_panel(&ui, &preview_state, &apply_armed);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_sysenv_interaction_request(move || {
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
        let system_path = system_path.clone();
        ui.on_sysenv_store_request(move |preset_name| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);

            let preset_name =
                match normalize_preset_name(preset_name.as_str(), ui.get_language_index()) {
                    Ok(name) => name,
                    Err(err) => {
                        append_sysenv_status_log(&ui, "ERROR", &err);
                        return;
                    }
                };
            let variables = preview_state.borrow().snapshot();

            match store_sysenv_preset(&system_path, &preset_name, variables) {
                Ok(()) => {
                    ui.set_sysenv_preset_name(preset_name.clone().into());
                    append_sysenv_status_log(
                        &ui,
                        "INFO",
                        &tf(
                            ui.get_language_index(),
                            "sysenv.msg.store_success",
                            &[("name", &preset_name)],
                        ),
                    );
                }
                Err(err) => {
                    append_sysenv_status_log(
                        &ui,
                        "ERROR",
                        &tf(
                            ui.get_language_index(),
                            "sysenv.msg.store_failed",
                            &[("error", &err)],
                        ),
                    );
                }
            }
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let apply_armed = Rc::clone(&apply_armed);
        let system_path = system_path.clone();
        ui.on_sysenv_load_preset_request(move |preset_name| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);

            let preset_name =
                match normalize_preset_name(preset_name.as_str(), ui.get_language_index()) {
                    Ok(name) => name,
                    Err(err) => {
                        append_sysenv_status_log(&ui, "ERROR", &err);
                        return;
                    }
                };

            match load_sysenv_preset(&system_path, &preset_name, ui.get_language_index()) {
                Ok(data) => {
                    ui.set_sysenv_preset_name(preset_name.clone().into());

                    let mut state = preview_state.borrow_mut();
                    state.merge(data.variables);
                    let vars = state.snapshot();
                    drop(state);

                    apply_vars_to_ui(&ui, &vars);
                    append_sysenv_status_log(
                        &ui,
                        "INFO",
                        &tf(
                            ui.get_language_index(),
                            "sysenv.msg.load_success",
                            &[("name", &preset_name)],
                        ),
                    );
                }
                Err(err) => {
                    append_sysenv_status_log(
                        &ui,
                        "ERROR",
                        &tf(
                            ui.get_language_index(),
                            "sysenv.msg.load_failed",
                            &[("error", &err)],
                        ),
                    );
                }
            }
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_sysenv_load_system_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);
            reload_system_env_to_preview(&ui, &preview_state);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_sysenv_preview_request(move |value_path, variable_name| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);

            let value = value_path.as_str().trim().to_string();
            if value.is_empty() {
                append_sysenv_status_log(
                    &ui,
                    "ERROR",
                    &t(ui.get_language_index(), "sysenv.msg.value_required"),
                );
                return;
            }

            let name = variable_name.as_str().trim().to_string();
            if let Err(err) = validate_sysenv_variable_name(&name, ui.get_language_index()) {
                append_sysenv_status_log(&ui, "ERROR", &err);
                return;
            }

            let mut state = preview_state.borrow_mut();
            if let Err(err) = state.insert_new(
                name.clone(),
                sanitize_ui_text(&value),
                ui.get_language_index(),
            ) {
                append_sysenv_status_log(&ui, "INFO", &err);
                return;
            }

            let vars = state.snapshot();
            drop(state);
            apply_vars_to_ui(&ui, &vars);

            append_sysenv_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "sysenv.msg.preview_success",
                    &[("name", &name), ("value", &value)],
                ),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_sysenv_edit_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            show_sysenv_value_editor(&ui, &preview_state, &apply_armed, index);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_sysenv_remove_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            reset_apply_progress(&ui, &apply_armed);

            let mut state = preview_state.borrow_mut();
            let Some(name) = state.remove_row(index as usize) else {
                return;
            };

            let vars = state.snapshot();
            drop(state);
            apply_vars_to_ui(&ui, &vars);

            append_sysenv_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "sysenv.msg.row_removed",
                    &[("name", &name)],
                ),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let apply_armed = Rc::clone(&apply_armed);
        ui.on_sysenv_commit_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let snapshot = preview_state.borrow().snapshot();

            if !*apply_armed.borrow() {
                if let Err(err) =
                    validate_sysenv_variable_snapshot(&snapshot, ui.get_language_index())
                {
                    append_sysenv_status_log(&ui, "ERROR", &err);
                    *apply_armed.borrow_mut() = false;
                    return;
                }

                let Some(system_vars) = sysenv_result_or_log(&ui, read_system_env_variables())
                else {
                    return;
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

                // Variables present in system but absent from preview will be deleted
                let delete_count = system_vars
                    .keys()
                    .filter(|name| !snapshot.contains_key(*name))
                    .count();

                if snapshot.is_empty() && delete_count == 0 {
                    append_sysenv_status_log(
                        &ui,
                        "ERROR",
                        &t(ui.get_language_index(), "sysenv.msg.preview_empty"),
                    );
                    *apply_armed.borrow_mut() = false;
                    return;
                }

                if add_count == 0 && change_count == 0 && delete_count == 0 {
                    append_sysenv_status_log(
                        &ui,
                        "INFO",
                        &t(ui.get_language_index(), "sysenv.msg.apply_no_changes"),
                    );
                    *apply_armed.borrow_mut() = false;
                    return;
                }

                *apply_armed.borrow_mut() = true;
                append_sysenv_status_log(
                    &ui,
                    "WARN",
                    &tf(
                        ui.get_language_index(),
                        "sysenv.msg.apply_confirm_pending",
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
            append_sysenv_status_log(
                &ui,
                "INFO",
                &t(ui.get_language_index(), "sysenv.msg.apply_confirm_execute"),
            );

            if let Err(err) = validate_sysenv_variable_snapshot(&snapshot, ui.get_language_index())
            {
                append_sysenv_status_log(&ui, "ERROR", &err);
                return;
            }

            let Some(system_vars) = sysenv_result_or_log(&ui, read_system_env_variables()) else {
                return;
            };

            let Some(system_value_types) =
                sysenv_result_or_log(&ui, read_system_env_variable_types())
            else {
                return;
            };

            // Write/update: entries that differ from current system state
            let targets = snapshot
                .iter()
                .filter_map(|(name, value)| {
                    (system_vars.get(name) != Some(value)).then_some((name.clone(), value.clone()))
                })
                .collect::<Vec<_>>();

            // Delete: variables present in system but removed from preview
            let delete_targets = system_vars
                .keys()
                .filter(|name| !snapshot.contains_key(*name))
                .cloned()
                .collect::<Vec<_>>();

            if targets.is_empty() && delete_targets.is_empty() {
                append_sysenv_status_log(
                    &ui,
                    "INFO",
                    &t(ui.get_language_index(), "sysenv.msg.apply_no_changes"),
                );
                return;
            }

            let mut ok_count = 0usize;
            let mut fail_count = 0usize;
            for (name, value) in &targets {
                match write_system_env_variable(name, value, system_value_types.get(name)) {
                    Ok(()) => ok_count += 1,
                    Err(err) => {
                        fail_count += 1;
                        append_sysenv_status_log(
                            &ui,
                            "ERROR",
                            &tf(
                                ui.get_language_index(),
                                "sysenv.msg.apply_item_failed",
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
                        append_sysenv_status_log(
                            &ui,
                            "ERROR",
                            &tf(
                                ui.get_language_index(),
                                "sysenv.msg.apply_delete_failed",
                                &[("name", name), ("error", &err)],
                            ),
                        );
                    }
                }
            }

            append_sysenv_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "sysenv.msg.apply_preview_done",
                    &[
                        ("ok", &ok_count.to_string()),
                        ("failed", &fail_count.to_string()),
                    ],
                ),
            );
        });
    }
}
