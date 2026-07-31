use std::cell::RefCell;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, VecModel};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_SYSTEM, GetFileAttributesW,
    INVALID_FILE_ATTRIBUTES, SetFileAttributesW,
};
use windows_sys::Win32::UI::Shell::{
    SHCNE_ATTRIBUTES, SHCNE_UPDATEDIR, SHCNE_UPDATEITEM, SHCNF_PATHW, SHChangeNotify,
};

use crate::core::util::append_log_line;
use crate::public::assets::lang::{sanitize_ui_text, t, tf};
use crate::public::config::{
    general::{FolderStylePresetDat, read_general_dat_path, write_general_dat_path},
    general_dat_path,
};
use crate::{FolderStyleEditorWindow, FolderStylePreviewRow, MainWindow};

#[derive(Clone, Copy)]
enum IniEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

#[derive(Clone)]
struct FolderStyleDraft {
    folder_path: PathBuf,
    original_content: Option<String>,
    content: String,
    encoding: IniEncoding,
}

#[derive(Default)]
struct FolderStylePreviewState {
    drafts: Vec<FolderStyleDraft>,
}

impl FolderStylePreviewState {
    fn snapshot(&self) -> Vec<FolderStyleDraft> {
        self.drafts.clone()
    }

    fn len(&self) -> usize {
        self.drafts.len()
    }

    fn is_empty(&self) -> bool {
        self.drafts.is_empty()
    }

    fn replace(&mut self, drafts: Vec<FolderStyleDraft>) {
        self.drafts = drafts;
    }

    fn has_folder_key(&self, folder_key: &str) -> bool {
        self.drafts
            .iter()
            .any(|draft| normalized_folder_key(&draft.folder_path) == folder_key)
    }

    fn push(&mut self, draft: FolderStyleDraft) -> Vec<FolderStyleDraft> {
        self.drafts.push(draft);
        self.snapshot()
    }

    fn move_up(&mut self, row_index: usize) -> Option<(String, Vec<FolderStyleDraft>)> {
        if row_index == 0 || row_index >= self.drafts.len() {
            return None;
        }

        self.drafts.swap(row_index - 1, row_index);
        let folder_text = self.drafts[row_index - 1].folder_path.display().to_string();
        Some((folder_text, self.snapshot()))
    }

    fn remove_row(
        &mut self,
        row_index: usize,
    ) -> Option<(FolderStyleDraft, Vec<FolderStyleDraft>)> {
        if row_index >= self.drafts.len() {
            return None;
        }

        let removed = self.drafts.remove(row_index);
        Some((removed, self.snapshot()))
    }

    fn clear(&mut self) {
        self.drafts.clear();
    }

    fn update_content(&mut self, row_index: usize, content: String) -> Option<String> {
        let draft = self.drafts.get_mut(row_index)?;
        let folder_text = draft.folder_path.display().to_string();
        draft.content = content;
        Some(folder_text)
    }
}

fn append_folderstyle_status_log(ui: &MainWindow, _level: &str, message: &str) {
    ui.set_folderstyle_status_text(
        append_log_line(ui.get_folderstyle_status_text().as_ref(), message).into(),
    );
}

fn folderstyle_config_path(app_dir: &Path) -> PathBuf {
    general_dat_path(app_dir)
}

fn normalize_preset_name(raw_name: &str, language_index: i32) -> Result<String, String> {
    let name = sanitize_ui_text(raw_name.trim());
    if name.is_empty() {
        Err(t(language_index, "folderstyle.msg.preset_name_required"))
    } else {
        Ok(name)
    }
}

fn store_folderstyle_preset(
    path: &Path,
    preset_name: &str,
    drafts: &[FolderStyleDraft],
) -> Result<(), String> {
    let mut data = read_general_dat_path(path)?;
    let preset = FolderStylePresetDat {
        folders: drafts
            .iter()
            .map(|draft| sanitize_ui_text(&draft.folder_path.to_string_lossy()))
            .collect(),
    };
    data.folderstyle
        .presets
        .insert(preset_name.to_string(), preset);
    write_general_dat_path(path, &data)
}

fn load_folderstyle_preset(
    path: &Path,
    preset_name: &str,
    language_index: i32,
) -> Result<FolderStylePresetDat, String> {
    let data = read_general_dat_path(path)?;
    data.folderstyle
        .presets
        .get(preset_name)
        .cloned()
        .ok_or_else(|| {
            tf(
                language_index,
                "folderstyle.msg.preset_not_found",
                &[("name", preset_name)],
            )
        })
}

fn desktop_ini_path(folder: &Path) -> PathBuf {
    folder.join("desktop.ini")
}

fn default_desktop_ini_content() -> String {
    "[.ShellClassInfo]\n".to_string()
}

fn normalized_folder_key(path: &Path) -> String {
    let key_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut text = key_path.to_string_lossy().replace('/', "\\").to_lowercase();
    if let Some(stripped) = text.strip_prefix("\\\\?\\unc\\") {
        text = format!("\\\\{}", stripped);
    } else if let Some(stripped) = text.strip_prefix("\\\\?\\") {
        text = stripped.to_string();
    }
    while text.len() > 3 && text.ends_with('\\') {
        text.pop();
    }
    text
}

fn validate_folder_path(raw_path: &str, language_index: i32) -> Result<PathBuf, String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(t(language_index, "folderstyle.msg.folder_required"));
    }

    let path = PathBuf::from(trimmed);
    if !path.exists() {
        return Err(t(language_index, "folderstyle.msg.folder_not_exists"));
    }
    if !path.is_dir() {
        return Err(t(language_index, "folderstyle.msg.not_folder"));
    }

    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|current_dir| current_dir.join(path))
            .map_err(|err| {
                let error = err.to_string();
                tf(
                    language_index,
                    "folderstyle.msg.resolve_failed",
                    &[("error", &error)],
                )
            })
    }
}

fn utf16_units_from_bytes(bytes: &[u8], little_endian: bool) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect()
}

fn decode_desktop_ini(bytes: &[u8]) -> (String, IniEncoding) {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return (
            String::from_utf8_lossy(&bytes[3..]).into_owned(),
            IniEncoding::Utf8Bom,
        );
    }

    if bytes.starts_with(&[0xff, 0xfe]) {
        let units = utf16_units_from_bytes(&bytes[2..], true);
        return (String::from_utf16_lossy(&units), IniEncoding::Utf16Le);
    }

    if bytes.starts_with(&[0xfe, 0xff]) {
        let units = utf16_units_from_bytes(&bytes[2..], false);
        return (String::from_utf16_lossy(&units), IniEncoding::Utf16Be);
    }

    (
        String::from_utf8_lossy(bytes).into_owned(),
        IniEncoding::Utf8,
    )
}

fn encode_desktop_ini(content: &str, encoding: IniEncoding) -> Vec<u8> {
    match encoding {
        IniEncoding::Utf8 => content.as_bytes().to_vec(),
        IniEncoding::Utf8Bom => {
            let mut out = vec![0xef, 0xbb, 0xbf];
            out.extend_from_slice(content.as_bytes());
            out
        }
        IniEncoding::Utf16Le => {
            let mut out = vec![0xff, 0xfe];
            for unit in content.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
        IniEncoding::Utf16Be => {
            let mut out = vec![0xfe, 0xff];
            for unit in content.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            out
        }
    }
}

fn load_folder_draft(
    folder_path: PathBuf,
    language_index: i32,
) -> Result<FolderStyleDraft, String> {
    let desktop_ini = desktop_ini_path(&folder_path);
    if !desktop_ini.exists() {
        return Ok(FolderStyleDraft {
            folder_path,
            original_content: None,
            content: default_desktop_ini_content(),
            encoding: IniEncoding::Utf16Le,
        });
    }

    if !desktop_ini.is_file() {
        let path_text = desktop_ini.display().to_string();
        return Err(tf(
            language_index,
            "folderstyle.msg.desktop_ini_not_file",
            &[("path", &path_text)],
        ));
    }

    let bytes = fs::read(&desktop_ini).map_err(|err| {
        let path_text = desktop_ini.display().to_string();
        let error = err.to_string();
        tf(
            language_index,
            "folderstyle.msg.read_failed",
            &[("path", &path_text), ("error", &error)],
        )
    })?;
    let (content, encoding) = decode_desktop_ini(&bytes);
    let content = sanitize_ui_text(&content);

    Ok(FolderStyleDraft {
        folder_path,
        original_content: Some(content.clone()),
        content,
        encoding,
    })
}

fn load_preset_drafts(
    ui: &MainWindow,
    path: &Path,
    preset_name: &str,
) -> Result<Vec<FolderStyleDraft>, String> {
    let language_index = ui.get_language_index();
    let preset_data = load_folderstyle_preset(path, preset_name, language_index)?;

    let mut drafts = Vec::new();
    for folder in preset_data.folders {
        let draft = validate_folder_path(&folder, language_index)
            .and_then(|folder_path| load_folder_draft(folder_path, language_index));
        match draft {
            Ok(draft) => drafts.push(draft),
            Err(err) => {
                append_folderstyle_status_log(
                    ui,
                    "ERROR",
                    &tf(
                        language_index,
                        "folderstyle.msg.preset_folder_load_failed",
                        &[("name", preset_name), ("path", &folder), ("error", &err)],
                    ),
                );
            }
        }
    }

    Ok(drafts)
}

fn refresh_preview(ui: &MainWindow, preview_state: &Rc<RefCell<FolderStylePreviewState>>) {
    let state = preview_state.borrow();
    set_preview_rows(ui, &state.drafts);
    ui.set_folderstyle_preview_text("".into());
}

fn append_preset_loaded_log(ui: &MainWindow, preset_name: &str, count: usize) {
    append_folderstyle_status_log(
        ui,
        "INFO",
        &tf(
            ui.get_language_index(),
            "folderstyle.msg.load_success",
            &[("name", preset_name), ("count", &count.to_string())],
        ),
    );
}

fn folder_status_key(draft: &FolderStyleDraft) -> &'static str {
    match &draft.original_content {
        None => "folderstyle.status.new",
        Some(original) if *original == draft.content => "folderstyle.status.loaded",
        Some(_) => "folderstyle.status.edited",
    }
}

fn desktop_ini_info_tip(content: &str) -> String {
    let mut in_shell_class_info = false;
    let mut fallback = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let section = line[1..line.len() - 1].trim();
            in_shell_class_info = section.eq_ignore_ascii_case(".ShellClassInfo");
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if key.trim().eq_ignore_ascii_case("InfoTip") {
            let info_tip = sanitize_ui_text(value.trim());
            if in_shell_class_info {
                return info_tip;
            }
            fallback.get_or_insert(info_tip);
        }
    }

    fallback.unwrap_or_default()
}

fn set_preview_rows(ui: &MainWindow, drafts: &[FolderStyleDraft]) {
    let language_index = ui.get_language_index();
    let rows = drafts
        .iter()
        .map(|draft| FolderStylePreviewRow {
            folder_path: sanitize_ui_text(&draft.folder_path.to_string_lossy()).into(),
            info_tip_text: desktop_ini_info_tip(&draft.content).into(),
            status_text: t(language_index, folder_status_key(draft)).into(),
        })
        .collect::<Vec<_>>();

    ui.set_folderstyle_preview_rows(ModelRc::new(VecModel::from(rows)));
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn get_path_attributes(path: &Path) -> Result<u32, String> {
    let wide = wide_path(path);
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == INVALID_FILE_ATTRIBUTES {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(attrs)
    }
}

fn set_path_attributes(path: &Path, attrs: u32) -> Result<(), String> {
    let wide = wide_path(path);
    let ok = unsafe { SetFileAttributesW(wide.as_ptr(), attrs) };
    if ok == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

fn write_desktop_ini(draft: &FolderStyleDraft) -> Result<(), String> {
    let desktop_ini = desktop_ini_path(&draft.folder_path);

    if desktop_ini.exists()
        && let Ok(attrs) = get_path_attributes(&desktop_ini)
        && attrs & FILE_ATTRIBUTE_READONLY != 0
    {
        set_path_attributes(&desktop_ini, attrs & !FILE_ATTRIBUTE_READONLY)?;
    }

    fs::write(
        &desktop_ini,
        encode_desktop_ini(&draft.content, draft.encoding),
    )
    .map_err(|err| err.to_string())?;

    let file_attrs = get_path_attributes(&desktop_ini).unwrap_or(0);
    set_path_attributes(
        &desktop_ini,
        file_attrs | FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM,
    )?;

    let folder_attrs = get_path_attributes(&draft.folder_path)?;
    set_path_attributes(&draft.folder_path, folder_attrs | FILE_ATTRIBUTE_READONLY)?;

    notify_shell_updated(&desktop_ini);
    notify_shell_updated(&draft.folder_path);
    Ok(())
}

fn notify_shell_updated(path: &Path) {
    let wide = wide_path(path);
    let item = wide.as_ptr() as *const core::ffi::c_void;
    unsafe {
        SHChangeNotify(SHCNE_UPDATEITEM as i32, SHCNF_PATHW, item, std::ptr::null());
        SHChangeNotify(SHCNE_ATTRIBUTES as i32, SHCNF_PATHW, item, std::ptr::null());
        SHChangeNotify(SHCNE_UPDATEDIR as i32, SHCNF_PATHW, item, std::ptr::null());
    }
}

fn apply_editor_window_config(editor: &FolderStyleEditorWindow, ui: &MainWindow) {
    let scale_factor = ui.window().scale_factor();
    let parent_size = ui.window().size().to_logical(scale_factor);
    let parent_position = ui.window().position().to_logical(scale_factor);
    let editor_width = parent_size.width * 3.0 / 4.0;
    let editor_height = parent_size.height * 3.0 / 4.0;
    let editor_x = parent_position.x + (parent_size.width - editor_width) / 2.0;
    let editor_y = parent_position.y + (parent_size.height - editor_height) / 2.0;

    editor
        .window()
        .set_size(slint::LogicalSize::new(editor_width, editor_height));
    editor
        .window()
        .set_position(slint::LogicalPosition::new(editor_x, editor_y));
}

fn schedule_delayed_editor_window_config_apply(
    editor: &FolderStyleEditorWindow,
    ui: &MainWindow,
    delay: Duration,
) {
    let editor_handle = editor.as_weak();
    let ui_handle = ui.as_weak();
    slint::Timer::single_shot(delay, move || {
        if let (Some(editor), Some(ui)) = (editor_handle.upgrade(), ui_handle.upgrade()) {
            apply_editor_window_config(&editor, &ui);
        }
    });
}

fn schedule_editor_window_config_apply(editor: &FolderStyleEditorWindow, ui: &MainWindow) {
    apply_editor_window_config(editor, ui);
    schedule_delayed_editor_window_config_apply(editor, ui, Duration::from_millis(0));
    schedule_delayed_editor_window_config_apply(editor, ui, Duration::from_millis(60));
}

fn show_folderstyle_editor(
    ui: &MainWindow,
    preview_state: &Rc<RefCell<FolderStylePreviewState>>,
    index: i32,
) {
    let row_index = index as usize;
    let Some(draft) = preview_state.borrow().drafts.get(row_index).cloned() else {
        return;
    };

    let editor = match FolderStyleEditorWindow::new() {
        Ok(editor) => editor,
        Err(err) => {
            append_folderstyle_status_log(ui, "ERROR", &err.to_string());
            return;
        }
    };

    editor.set_folder_path(sanitize_ui_text(&draft.folder_path.to_string_lossy()).into());
    editor.set_ini_content(draft.content.into());
    editor.set_language_index(ui.get_language_index());
    editor.on_tr(|key, language_index| t(language_index, key.as_str()).into());

    let editor_lifetime: Rc<RefCell<Option<FolderStyleEditorWindow>>> = Rc::new(RefCell::new(None));

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
        editor.on_accept_request(move |content| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let mut state = preview_state.borrow_mut();

            let Some(folder_text) =
                state.update_content(row_index, sanitize_ui_text(content.as_str()))
            else {
                if let Some(editor) = editor_handle.upgrade() {
                    let _ = editor.hide();
                }
                editor_lifetime.borrow_mut().take();
                return;
            };

            let snapshot = state.snapshot();
            drop(state);
            set_preview_rows(&ui, &snapshot);

            append_folderstyle_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "folderstyle.msg.row_edited",
                    &[("path", &folder_text)],
                ),
            );

            if let Some(editor) = editor_handle.upgrade() {
                let _ = editor.hide();
            }
            editor_lifetime.borrow_mut().take();
        });
    }

    let _ = editor.show();
    schedule_editor_window_config_apply(&editor, ui);
    *editor_lifetime.borrow_mut() = Some(editor);
}

pub fn setup_folderstyle_handlers(ui: &MainWindow, app_dir: &Path) {
    let folderstyle_path = folderstyle_config_path(app_dir);
    let preview_state: Rc<RefCell<FolderStylePreviewState>> =
        Rc::new(RefCell::new(FolderStylePreviewState::default()));

    ui.set_folderstyle_status_text("".into());
    ui.set_folderstyle_preview_text("".into());
    ui.set_folderstyle_preview_rows(ModelRc::new(VecModel::from(
        Vec::<FolderStylePreviewRow>::new(),
    )));
    append_folderstyle_status_log(
        ui,
        "INFO",
        &t(ui.get_language_index(), "folderstyle.msg.ready"),
    );

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        let folderstyle_path = folderstyle_path.clone();
        ui.on_folderstyle_store_request(move |preset_name| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let preset_name =
                match normalize_preset_name(preset_name.as_str(), ui.get_language_index()) {
                    Ok(name) => name,
                    Err(err) => {
                        append_folderstyle_status_log(&ui, "ERROR", &err);
                        return;
                    }
                };

            let snapshot = preview_state.borrow().snapshot();
            match store_folderstyle_preset(&folderstyle_path, &preset_name, &snapshot) {
                Ok(()) => {
                    ui.set_folderstyle_preset_name(preset_name.clone().into());
                    append_folderstyle_status_log(
                        &ui,
                        "INFO",
                        &tf(
                            ui.get_language_index(),
                            "folderstyle.msg.store_success",
                            &[("name", &preset_name)],
                        ),
                    );
                }
                Err(err) => {
                    append_folderstyle_status_log(
                        &ui,
                        "ERROR",
                        &tf(
                            ui.get_language_index(),
                            "folderstyle.msg.store_failed",
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
        let folderstyle_path = folderstyle_path.clone();
        ui.on_folderstyle_load_preset_request(move |preset_name| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let preset_name =
                match normalize_preset_name(preset_name.as_str(), ui.get_language_index()) {
                    Ok(name) => name,
                    Err(err) => {
                        append_folderstyle_status_log(&ui, "ERROR", &err);
                        return;
                    }
                };

            match load_preset_drafts(&ui, &folderstyle_path, &preset_name) {
                Ok(drafts) => {
                    preview_state.borrow_mut().replace(drafts);
                    refresh_preview(&ui, &preview_state);
                    let count = preview_state.borrow().len();
                    ui.set_folderstyle_preset_name(preset_name.clone().into());
                    append_preset_loaded_log(&ui, &preset_name, count);
                }
                Err(err) => {
                    append_folderstyle_status_log(
                        &ui,
                        "ERROR",
                        &tf(
                            ui.get_language_index(),
                            "folderstyle.msg.load_failed",
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
        ui.on_folderstyle_add_request(move |folder| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let language_index = ui.get_language_index();
            let folder_path = match validate_folder_path(folder.as_str(), language_index) {
                Ok(path) => path,
                Err(err) => {
                    append_folderstyle_status_log(&ui, "ERROR", &err);
                    return;
                }
            };

            let folder_key = normalized_folder_key(&folder_path);
            if preview_state.borrow().has_folder_key(&folder_key) {
                let path_text = folder_path.display().to_string();
                append_folderstyle_status_log(
                    &ui,
                    "INFO",
                    &tf(
                        language_index,
                        "folderstyle.msg.folder_already_exists",
                        &[("path", &path_text)],
                    ),
                );
                return;
            }

            match load_folder_draft(folder_path.clone(), language_index) {
                Ok(draft) => {
                    let path_text = folder_path.display().to_string();
                    ui.set_folderstyle_folder_path(sanitize_ui_text(&path_text).into());

                    let snapshot = preview_state.borrow_mut().push(draft);

                    set_preview_rows(&ui, &snapshot);
                    ui.set_folderstyle_preview_text("".into());

                    append_folderstyle_status_log(
                        &ui,
                        "INFO",
                        &tf(
                            language_index,
                            "folderstyle.msg.preview_added",
                            &[("path", &path_text)],
                        ),
                    );
                }
                Err(err) => {
                    append_folderstyle_status_log(&ui, "ERROR", &err);
                }
            }
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        ui.on_folderstyle_move_up_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let (folder_text, snapshot) = {
                let mut state = preview_state.borrow_mut();
                let Some(result) = state.move_up(index as usize) else {
                    return;
                };
                result
            };

            set_preview_rows(&ui, &snapshot);
            append_folderstyle_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "folderstyle.msg.row_moved_up",
                    &[("path", &folder_text)],
                ),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        ui.on_folderstyle_edit_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            show_folderstyle_editor(&ui, &preview_state, index);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        ui.on_folderstyle_remove_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let (removed, snapshot) = {
                let mut state = preview_state.borrow_mut();
                let Some(result) = state.remove_row(index as usize) else {
                    return;
                };
                result
            };

            set_preview_rows(&ui, &snapshot);

            let path_text = removed.folder_path.display().to_string();
            append_folderstyle_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "folderstyle.msg.row_removed",
                    &[("path", &path_text)],
                ),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        ui.on_folderstyle_clear_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            preview_state.borrow_mut().clear();

            ui.set_folderstyle_preview_text("".into());
            ui.set_folderstyle_preview_rows(ModelRc::new(VecModel::from(Vec::<
                FolderStylePreviewRow,
            >::new())));
            append_folderstyle_status_log(
                &ui,
                "INFO",
                &t(ui.get_language_index(), "folderstyle.msg.cleared"),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let preview_state = Rc::clone(&preview_state);
        ui.on_folderstyle_apply_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let mut state = preview_state.borrow_mut();

            if state.is_empty() {
                append_folderstyle_status_log(
                    &ui,
                    "ERROR",
                    &t(ui.get_language_index(), "folderstyle.msg.preview_empty"),
                );
                return;
            }

            let mut ok_count = 0usize;
            let mut fail_count = 0usize;
            for draft in state.drafts.iter_mut() {
                match write_desktop_ini(draft) {
                    Ok(()) => {
                        ok_count += 1;
                        draft.original_content = Some(draft.content.clone());
                    }
                    Err(err) => {
                        fail_count += 1;
                        let path_text = draft.folder_path.display().to_string();
                        append_folderstyle_status_log(
                            &ui,
                            "ERROR",
                            &tf(
                                ui.get_language_index(),
                                "folderstyle.msg.apply_item_failed",
                                &[("path", &path_text), ("error", &err)],
                            ),
                        );
                    }
                }
            }

            set_preview_rows(&ui, &state.drafts);
            append_folderstyle_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "folderstyle.msg.apply_done",
                    &[
                        ("ok", &ok_count.to_string()),
                        ("failed", &fail_count.to_string()),
                    ],
                ),
            );
        });
    }
}
