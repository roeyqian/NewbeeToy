use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::os::windows::ffi::OsStrExt;

use slint::{ComponentHandle, ModelRc, VecModel};

use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    EnumResourceNamesW, FindResourceW, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
    LoadLibraryExW, LoadResource, LockResource, SizeofResource,
};
use windows_sys::core::BOOL;

use crate::core::lang::{sanitize_ui_text, t, tf};
use crate::{IconPreviewRow, MainWindow};

#[derive(Clone)]
struct IconCandidate {
    source_path: PathBuf,
    output_name: String,
    is_ico_file: bool,
    is_extractable: bool,
}

#[derive(Clone)]
struct PreviewRow {
    source_name: String,
    ico_name: String,
    status_text: String,
    has_error: bool,
}

#[derive(Clone)]
struct IconState {
    candidates: Vec<IconCandidate>,
    excluded_indices: HashSet<usize>,
}

fn normalize_name(name: &str) -> String {
    let mut normalized = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            normalized.push(ch);
        } else {
            normalized.push('_');
        }
    }

    if normalized.is_empty() {
        "icon".to_string()
    } else {
        normalized
    }
}

fn build_candidate(path: &Path) -> Option<IconCandidate> {
    let ext = path
        .extension()
        .map(|x| x.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if ext == "ico" {
        let file_name = path
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_else(|| "icon.ico".to_string());
        return Some(IconCandidate {
            source_path: path.to_path_buf(),
            output_name: file_name,
            is_ico_file: true,
            is_extractable: true,
        });
    }

    if ext == "exe" || ext == "dll" || ext == "icl" || ext == "lnk" {
        let stem = path
            .file_stem()
            .map(|x| normalize_name(&x.to_string_lossy()))
            .unwrap_or_else(|| "icon".to_string());
        return Some(IconCandidate {
            source_path: path.to_path_buf(),
            output_name: format!("{}.ico", stem),
            is_ico_file: false,
            is_extractable: true,
        });
    }

    None
}

fn collect_extractable_candidates(source_path: &Path) -> Result<Vec<IconCandidate>, String> {
    let mut candidates = Vec::new();

    if source_path.is_dir() {
        let read_dir = fs::read_dir(source_path).map_err(|e| e.to_string())?;
        for entry in read_dir {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(candidate) = build_candidate(&path) {
                candidates.push(candidate);
            }
        }
    } else if source_path.is_file() {
        if let Some(candidate) = build_candidate(source_path) {
            candidates.push(candidate);
        }
    } else {
        return Err("Input path does not exist".to_string());
    }

    candidates.sort_by_key(|x| {
        x.source_path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });

    Ok(candidates)
}

fn append_icon_status_log(ui: &MainWindow, level: &str, message: &str) {
    let current = ui.get_icon_status_text().to_string();
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

    ui.set_icon_status_text(lines.join("\n").into());
}

#[derive(Clone, Copy)]
struct IconDirEntry {
    width: u8,
    height: u8,
    color_count: u8,
    reserved: u8,
    planes: u16,
    bit_count: u16,
    resource_id: u16,
}

struct LoadedModule(HMODULE);

impl Drop for LoadedModule {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                FreeLibrary(self.0);
            }
        }
    }
}

fn probe_candidate_extractable(candidate: &IconCandidate) -> bool {
    if candidate.is_ico_file {
        return fs::File::open(&candidate.source_path).is_ok();
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let probe_name = format!("newbeetoy_probe_{}_{}.ico", std::process::id(), stamp);
    let probe_path = std::env::temp_dir().join(probe_name);

    let ok = extract_associated_icon_to_ico(&candidate.source_path, &probe_path).is_ok();
    let _ = fs::remove_file(&probe_path);
    ok
}

fn make_unique_output_path(output_dir: &Path, base_name: &str) -> PathBuf {
    let mut candidate = output_dir.join(base_name);
    if !candidate.exists() {
        return candidate;
    }

    let base = Path::new(base_name)
        .file_stem()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| "icon".to_string());
    let ext = Path::new(base_name)
        .extension()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| "ico".to_string());

    let mut index = 2usize;
    loop {
        candidate = output_dir.join(format!("{}_{}.{}", base, index, ext));
        if !candidate.exists() {
            return candidate;
        }
        index += 1;
    }
}

fn extract_associated_icon_to_ico(source: &Path, destination: &Path) -> Result<(), String> {
    unsafe extern "system" fn enum_group_icons(
        _module: HMODULE,
        _resource_type: *const u16,
        name: *const u16,
        lparam: isize,
    ) -> BOOL {
        let ids = unsafe { &mut *(lparam as *mut Vec<u16>) };
        let raw = name as usize;
        if (raw >> 16) == 0 {
            ids.push(raw as u16);
        }
        1
    }

    fn load_resource_bytes(
        module: HMODULE,
        res_type: *const u16,
        res_name: *const u16,
    ) -> Result<Vec<u8>, String> {
        let hres = unsafe { FindResourceW(module, res_name, res_type) };
        if hres.is_null() {
            return Err("FindResourceW failed".to_string());
        }
        let hglob = unsafe { LoadResource(module, hres) };
        if hglob.is_null() {
            return Err("LoadResource failed".to_string());
        }
        let size = unsafe { SizeofResource(module, hres) as usize };
        if size == 0 {
            return Err("Resource size is zero".to_string());
        }
        let ptr = unsafe { LockResource(hglob) as *const u8 };
        if ptr.is_null() {
            return Err("LockResource failed".to_string());
        }
        Ok(unsafe { std::slice::from_raw_parts(ptr, size).to_vec() })
    }

    fn parse_group_entries(group_data: &[u8]) -> Result<Vec<IconDirEntry>, String> {
        const GROUP_HEADER_SIZE: usize = 6;
        const GROUP_ENTRY_SIZE: usize = 14;

        if group_data.len() < GROUP_HEADER_SIZE {
            return Err("Invalid group icon header".to_string());
        }

        let kind = u16::from_le_bytes([group_data[2], group_data[3]]);
        if kind != 1 {
            return Err("Unsupported group icon kind".to_string());
        }

        let count = u16::from_le_bytes([group_data[4], group_data[5]]) as usize;
        let need = GROUP_HEADER_SIZE + count * GROUP_ENTRY_SIZE;
        if group_data.len() < need {
            return Err("Group icon data is truncated".to_string());
        }

        let mut entries = Vec::with_capacity(count);
        for idx in 0..count {
            let start = GROUP_HEADER_SIZE + idx * GROUP_ENTRY_SIZE;
            entries.push(IconDirEntry {
                width: group_data[start],
                height: group_data[start + 1],
                color_count: group_data[start + 2],
                reserved: group_data[start + 3],
                planes: u16::from_le_bytes([group_data[start + 4], group_data[start + 5]]),
                bit_count: u16::from_le_bytes([group_data[start + 6], group_data[start + 7]]),
                resource_id: u16::from_le_bytes([group_data[start + 12], group_data[start + 13]]),
            });
        }
        Ok(entries)
    }

    fn write_ico_file(
        output_path: &Path,
        entries: &[IconDirEntry],
        icon_blobs: &[Vec<u8>],
    ) -> Result<(), String> {
        let count = entries.len();
        let header_size = 6usize;
        let dir_entry_size = 16usize;
        let mut offset = (header_size + dir_entry_size * count) as u32;

        let mut out = Vec::new();
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&(count as u16).to_le_bytes());

        for (entry, blob) in entries.iter().zip(icon_blobs.iter()) {
            out.push(entry.width);
            out.push(entry.height);
            out.push(entry.color_count);
            out.push(entry.reserved);
            out.extend_from_slice(&entry.planes.to_le_bytes());
            out.extend_from_slice(&entry.bit_count.to_le_bytes());
            out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            offset += blob.len() as u32;
        }

        for blob in icon_blobs {
            out.extend_from_slice(blob);
        }

        fs::write(output_path, out).map_err(|e| e.to_string())
    }

    let wide_path = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();

    let flags = LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE;
    let module = unsafe { LoadLibraryExW(wide_path.as_ptr(), std::ptr::null_mut(), flags) };
    if module.is_null() {
        return Err(format!("Failed to load module: {}", source.display()));
    }
    let module = LoadedModule(module);

    let mut group_ids = Vec::<u16>::new();
    let rt_group_icon = 14usize as *const u16;
    unsafe {
        EnumResourceNamesW(
            module.0,
            rt_group_icon,
            Some(enum_group_icons),
            &mut group_ids as *mut Vec<u16> as isize,
        );
    }
    if group_ids.is_empty() {
        return Err(format!("No RT_GROUP_ICON in {}", source.display()));
    }

    group_ids.sort_unstable();
    let group_id = group_ids[0];

    let group_data = load_resource_bytes(module.0, rt_group_icon, group_id as usize as *const u16)?;
    let entries = parse_group_entries(&group_data)?;
    if entries.is_empty() {
        return Err("Icon group has no entries".to_string());
    }

    let mut blobs = Vec::with_capacity(entries.len());
    let rt_icon = 3usize as *const u16;
    for entry in &entries {
        let blob = load_resource_bytes(module.0, rt_icon, entry.resource_id as usize as *const u16)?;
        blobs.push(blob);
    }

    write_ico_file(destination, &entries, &blobs)
}

fn set_preview_rows(ui: &MainWindow, rows: Vec<PreviewRow>) {
    let mapped = rows
        .into_iter()
        .map(|row| IconPreviewRow {
            source_name: sanitize_ui_text(&row.source_name).into(),
            ico_name: sanitize_ui_text(&row.ico_name).into(),
            status_text: sanitize_ui_text(&row.status_text).into(),
            has_error: row.has_error,
        })
        .collect::<Vec<_>>();

    ui.set_icon_preview_rows(ModelRc::new(VecModel::from(mapped)));
}

fn pending_rows_for_state(state: &IconState, lang_en: bool) -> Vec<PreviewRow> {
    state
        .candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            if state.excluded_indices.contains(&idx) {
                return None;
            }

            Some(PreviewRow {
                source_name: item
                    .source_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                ico_name: item.output_name.clone(),
                status_text: if item.is_extractable {
                    t(lang_en, "icon.status.extractable")
                } else {
                    t(lang_en, "icon.status.unextractable")
                },
                has_error: !item.is_extractable,
            })
        })
        .collect()
}

fn filtered_candidates(state: &IconState) -> Vec<IconCandidate> {
    state
        .candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            (!state.excluded_indices.contains(&idx) && item.is_extractable).then_some(item.clone())
        })
        .collect()
}

pub fn setup_icon_handlers(ui: &MainWindow) {
    let latest_icon_state: Rc<RefCell<Option<IconState>>> = Rc::new(RefCell::new(None));

    ui.set_icon_status_text("".into());
    ui.set_icon_preview_text("".into());
    ui.set_icon_preview_rows(ModelRc::new(VecModel::from(Vec::<IconPreviewRow>::new())));
    append_icon_status_log(ui, "INFO", &t(ui.get_lang_en(), "icon.msg.ready"));

    {
        let ui_handle = ui.as_weak();
        let icon_state = Rc::clone(&latest_icon_state);
        ui.on_icon_scan_request(move |source| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let source_path = PathBuf::from(source.as_str());
            if source_path.as_os_str().is_empty() {
                ui.set_icon_preview_text(t(ui.get_lang_en(), "icon.msg.choose_input_first").into());
                ui.set_icon_preview_rows(ModelRc::new(
                    VecModel::from(Vec::<IconPreviewRow>::new()),
                ));
                append_icon_status_log(
                    &ui,
                    "ERROR",
                    &t(ui.get_lang_en(), "icon.msg.choose_input_first"),
                );
                return;
            }

            if !source_path.exists() {
                ui.set_icon_preview_text(t(ui.get_lang_en(), "icon.msg.invalid_input_path").into());
                ui.set_icon_preview_rows(ModelRc::new(
                    VecModel::from(Vec::<IconPreviewRow>::new()),
                ));
                append_icon_status_log(
                    &ui,
                    "ERROR",
                    &t(ui.get_lang_en(), "icon.msg.invalid_input_path"),
                );
                return;
            }

            match collect_extractable_candidates(&source_path) {
                Ok(mut candidates) => {
                    if candidates.is_empty() {
                        ui.set_icon_preview_text(
                            t(ui.get_lang_en(), "icon.msg.no_extractable_files").into(),
                        );
                        ui.set_icon_preview_rows(ModelRc::new(VecModel::from(
                            Vec::<IconPreviewRow>::new(),
                        )));
                        *icon_state.borrow_mut() = None;
                        append_icon_status_log(
                            &ui,
                            "ERROR",
                            &t(ui.get_lang_en(), "icon.msg.no_extractable_files"),
                        );
                        return;
                    }

                    for candidate in &mut candidates {
                        candidate.is_extractable = probe_candidate_extractable(candidate);
                    }

                    let new_state = IconState {
                        candidates,
                        excluded_indices: HashSet::new(),
                    };
                    let extractable_count = new_state
                        .candidates
                        .iter()
                        .filter(|c| c.is_extractable)
                        .count()
                        .to_string();
                    let rows = pending_rows_for_state(&new_state, ui.get_lang_en());
                    ui.set_icon_preview_text("".into());
                    set_preview_rows(&ui, rows);
                    *icon_state.borrow_mut() = Some(new_state);
                    append_icon_status_log(
                        &ui,
                        "INFO",
                        &tf(
                            ui.get_lang_en(),
                            "icon.msg.scan_success",
                            &[("count", &extractable_count)],
                        ),
                    );

                    if extractable_count == "0" {
                        append_icon_status_log(
                            &ui,
                            "ERROR",
                            &t(ui.get_lang_en(), "icon.msg.no_extractable_files"),
                        );
                    }
                }
                Err(err) => {
                    ui.set_icon_preview_text(err.clone().into());
                    ui.set_icon_preview_rows(ModelRc::new(VecModel::from(
                        Vec::<IconPreviewRow>::new(),
                    )));
                    append_icon_status_log(
                        &ui,
                        "ERROR",
                        &tf(ui.get_lang_en(), "icon.msg.scan_failed", &[("error", &err)]),
                    );
                }
            }
        });
    }

    {
        let ui_handle = ui.as_weak();
        let icon_state = Rc::clone(&latest_icon_state);
        ui.on_icon_remove_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let mut borrowed = icon_state.borrow_mut();
            let Some(state) = borrowed.as_mut() else {
                append_icon_status_log(&ui, "ERROR", &t(ui.get_lang_en(), "icon.msg.scan_first"));
                return;
            };

            let visible_indices = state
                .candidates
                .iter()
                .enumerate()
                .filter_map(|(idx, _)| (!state.excluded_indices.contains(&idx)).then_some(idx))
                .collect::<Vec<_>>();

            let row_index = index as usize;
            if row_index >= visible_indices.len() {
                return;
            }

            let removed_idx = visible_indices[row_index];
            let removed_name = state.candidates[removed_idx]
                .source_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            state.excluded_indices.insert(removed_idx);

            let rows = pending_rows_for_state(state, ui.get_lang_en());
            set_preview_rows(&ui, rows);
            append_icon_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_lang_en(),
                    "icon.msg.row_removed",
                    &[("name", &removed_name)],
                ),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let icon_state = Rc::clone(&latest_icon_state);
        ui.on_icon_extract_request(move |output| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let output = output.as_str().trim().to_string();
            if output.is_empty() {
                append_icon_status_log(
                    &ui,
                    "ERROR",
                    &t(ui.get_lang_en(), "icon.msg.output_required"),
                );
                return;
            }

            let output_dir = PathBuf::from(output);
            if let Err(err) = fs::create_dir_all(&output_dir) {
                let err_text = err.to_string();
                append_icon_status_log(
                    &ui,
                    "ERROR",
                    &tf(
                        ui.get_lang_en(),
                        "icon.msg.create_output_dir_failed",
                        &[("error", &err_text)],
                    ),
                );
                return;
            }

            let selected = {
                let borrowed = icon_state.borrow();
                let Some(state) = borrowed.as_ref() else {
                    append_icon_status_log(&ui, "ERROR", &t(ui.get_lang_en(), "icon.msg.scan_first"));
                    return;
                };

                filtered_candidates(state)
            };

            if selected.is_empty() {
                append_icon_status_log(
                    &ui,
                    "INFO",
                    &t(ui.get_lang_en(), "icon.msg.no_selected_items"),
                );
                return;
            }

            let mut rows = Vec::with_capacity(selected.len());
            let mut success_count = 0usize;
            let mut failed_count = 0usize;

            for candidate in selected {
                let source_name = candidate
                    .source_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let destination = make_unique_output_path(&output_dir, &candidate.output_name);
                let ico_name = destination
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let result = if candidate.is_ico_file {
                    fs::copy(&candidate.source_path, &destination)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                } else {
                    extract_associated_icon_to_ico(&candidate.source_path, &destination)
                };

                match result {
                    Ok(()) => {
                        success_count += 1;
                        rows.push(PreviewRow {
                            source_name,
                            ico_name,
                            status_text: t(ui.get_lang_en(), "icon.status.extractable"),
                            has_error: false,
                        });
                    }
                    Err(_err) => {
                        failed_count += 1;
                        rows.push(PreviewRow {
                            source_name,
                            ico_name,
                            status_text: t(ui.get_lang_en(), "icon.status.unextractable"),
                            has_error: true,
                        });
                    }
                }
            }

            ui.set_icon_preview_text("".into());
            set_preview_rows(&ui, rows);

            let output_dir_text = output_dir.display().to_string();
            let success = success_count.to_string();
            append_icon_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_lang_en(),
                    "icon.msg.extract_done",
                    &[("count", &success), ("path", &output_dir_text)],
                ),
            );

            if failed_count > 0 {
                let failed = failed_count.to_string();
                append_icon_status_log(
                    &ui,
                    "ERROR",
                    &tf(
                        ui.get_lang_en(),
                        "icon.msg.extract_failed_summary",
                        &[("count", &failed)],
                    ),
                );
            }
        });
    }

    {
        let ui_handle = ui.as_weak();
        let icon_state = Rc::clone(&latest_icon_state);
        ui.on_icon_clear_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let mut borrowed = icon_state.borrow_mut();
            if let Some(state) = borrowed.as_mut() {
                state.excluded_indices = (0..state.candidates.len()).collect();
                set_preview_rows(&ui, pending_rows_for_state(state, ui.get_lang_en()));
            } else {
                ui.set_icon_preview_rows(ModelRc::new(VecModel::from(Vec::<IconPreviewRow>::new())));
            }
            ui.set_icon_preview_text("".into());
            append_icon_status_log(&ui, "INFO", &t(ui.get_lang_en(), "icon.msg.cleared"));
        });
    }
}

