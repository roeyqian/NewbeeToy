use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::core::lang::{sanitize_ui_text, t, tf};
use crate::core::util::append_log_line;
use crate::{MainWindow, UnlockPreviewRow};

#[derive(Clone)]
struct LockerInfo {
    process_name: String,
    pid: u32,
    is_system_process: bool,
    is_system_file: bool,
    note: String,
}

#[derive(Clone)]
struct UnlockState {
    target_key: String,
    lockers: Vec<LockerInfo>,
    excluded_indices: HashSet<usize>,
}

impl UnlockState {
    fn visible_indices(&self) -> Vec<usize> {
        self.lockers
            .iter()
            .enumerate()
            .filter_map(|(idx, _)| (!self.excluded_indices.contains(&idx)).then_some(idx))
            .collect()
    }

    fn filtered_lockers(&self) -> Vec<LockerInfo> {
        self.visible_indices()
            .into_iter()
            .map(|idx| self.lockers[idx].clone())
            .collect()
    }

    fn exclude_visible_row(&mut self, visible_row_index: usize) -> Option<String> {
        let visible_indices = self.visible_indices();
        let removed_idx = *visible_indices.get(visible_row_index)?;
        let removed_name = self.lockers[removed_idx].process_name.clone();
        self.excluded_indices.insert(removed_idx);
        Some(removed_name)
    }

    fn exclude_all_rows(&mut self) {
        self.excluded_indices = (0..self.lockers.len()).collect();
    }
}

#[derive(Clone, Copy)]
enum ScanError {
    Start(u32),
    Register(u32),
    GetList(u32),
}

const ACCESS_DENIED_CODE: u32 = 5;
const MAX_DIRECTORY_SCAN_FILES: usize = 256;
static WINDOWS_DIR_PREFIX: OnceLock<String> = OnceLock::new();

fn normalize_windows_path(path: &Path) -> String {
    trim_windows_path_tail(path.to_string_lossy().replace('/', "\\").to_lowercase())
}

fn trim_windows_path_tail(path: String) -> String {
    let mut text = path;
    while text.len() > 3 && text.ends_with('\\') {
        text.pop();
    }
    text
}

fn detect_windows_dir_prefix() -> String {
    for key in ["SystemRoot", "WINDIR"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return normalize_windows_path(Path::new(trimmed));
            }
        }
    }

    "c:\\windows".to_string()
}

fn is_windows_system_path(path: &Path) -> bool {
    let target = normalize_windows_path(path);
    let base = WINDOWS_DIR_PREFIX.get_or_init(detect_windows_dir_prefix);
    target == *base || target.starts_with(&(base.to_string() + "\\"))
}

fn append_unlock_status_log(ui: &MainWindow, _level: &str, message: &str) {
    ui.set_unlock_status_text(
        append_log_line(ui.get_unlock_status_text().as_ref(), message).into(),
    );
}

fn set_preview_rows(ui: &MainWindow, rows: Vec<LockerInfo>) {
    let mapped = rows
        .into_iter()
        .map(|row| UnlockPreviewRow {
            process_name: sanitize_ui_text(&row.process_name).into(),
            process_id: row.pid.to_string().into(),
            process_kind: if row.is_system_process {
                t(ui.get_language_index(), "unlock.process.system").into()
            } else {
                t(ui.get_language_index(), "unlock.process.general").into()
            },
            file_kind: if row.is_system_file {
                t(ui.get_language_index(), "unlock.file.system").into()
            } else {
                t(ui.get_language_index(), "unlock.file.general").into()
            },
            note: sanitize_ui_text(&row.note).into(),
            has_warning: row.is_system_process || row.is_system_file,
        })
        .collect::<Vec<_>>();

    ui.set_unlock_preview_rows(ModelRc::new(VecModel::from(mapped)));
}

fn apply_unlock_exclusions(ui: &MainWindow, state: &UnlockState) {
    set_preview_rows(ui, state.filtered_lockers());
}

fn utf16_to_string(raw: &[u16]) -> String {
    let end = raw.iter().position(|x| *x == 0).unwrap_or(raw.len());
    sanitize_ui_text(&String::from_utf16_lossy(&raw[..end]))
}

fn query_process_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }

    let mut buffer = vec![0u16; 32768];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) };
    unsafe {
        CloseHandle(handle);
    }

    if ok == 0 || size == 0 {
        None
    } else {
        Some(sanitize_ui_text(&String::from_utf16_lossy(
            &buffer[..size as usize],
        )))
    }
}

fn is_system_process(pid: u32, app_name: &str) -> bool {
    if pid == 0 || pid == 4 {
        return true;
    }

    let lower_name = app_name.to_lowercase();
    if lower_name == "system" || lower_name == "registry" {
        return true;
    }

    query_process_path(pid)
        .map(|path| is_windows_system_path(Path::new(&path)))
        .unwrap_or(false)
}

fn format_scan_error(language_index: i32, error: ScanError) -> String {
    match error {
        ScanError::Start(code) => tf(
            language_index,
            "unlock.msg.scan_failed_with_code",
            &[("code", &code.to_string())],
        ),
        ScanError::Register(code) => tf(
            language_index,
            "unlock.msg.register_failed_with_code",
            &[("code", &code.to_string())],
        ),
        ScanError::GetList(code) => tf(
            language_index,
            "unlock.msg.get_list_failed_with_code",
            &[("code", &code.to_string())],
        ),
    }
}

fn merge_lockers(merged: &mut HashMap<u32, LockerInfo>, row: LockerInfo) {
    if let Some(existing) = merged.get_mut(&row.pid) {
        existing.is_system_file |= row.is_system_file;
        existing.is_system_process |= row.is_system_process;
        return;
    }

    merged.insert(row.pid, row);
}

fn collect_directory_files(dir: &Path, limit: usize) -> (Vec<PathBuf>, bool) {
    let mut files = Vec::new();
    let mut queue = VecDeque::from([dir.to_path_buf()]);
    let mut has_permission_denied = false;

    while let Some(current) = queue.pop_front() {
        let entries = match std::fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(err) => {
                if err.kind() == ErrorKind::PermissionDenied {
                    has_permission_denied = true;
                }
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    if err.kind() == ErrorKind::PermissionDenied {
                        has_permission_denied = true;
                    }
                    continue;
                }
            };

            let path = entry.path();
            if path.is_dir() {
                queue.push_back(path);
                continue;
            }
            if path.is_file() {
                files.push(path);
                if files.len() >= limit {
                    return (files, has_permission_denied);
                }
            }
        }
    }

    (files, has_permission_denied)
}

fn scan_target_lockers(path: &Path, language_index: i32) -> Result<Vec<LockerInfo>, String> {
    if path.is_file() {
        return scan_lockers(path, language_index)
            .map_err(|err| format_scan_error(language_index, err));
    }

    let (files, read_permission_denied) = collect_directory_files(path, MAX_DIRECTORY_SCAN_FILES);
    if files.is_empty() {
        return if read_permission_denied {
            Err(t(language_index, "unlock.msg.directory_access_denied"))
        } else {
            Ok(Vec::new())
        };
    }

    let mut merged = HashMap::<u32, LockerInfo>::new();
    let mut has_successful_scan = false;
    let mut has_access_denied = read_permission_denied;

    for file in files {
        match scan_lockers(&file, language_index) {
            Ok(lockers) => {
                has_successful_scan = true;
                for locker in lockers {
                    merge_lockers(&mut merged, locker);
                }
            }
            Err(ScanError::Start(code) | ScanError::Register(code) | ScanError::GetList(code))
                if code == ACCESS_DENIED_CODE =>
            {
                has_access_denied = true;
            }
            Err(err) => return Err(format_scan_error(language_index, err)),
        }
    }

    if !has_successful_scan && has_access_denied {
        return Err(t(language_index, "unlock.msg.directory_access_denied"));
    }

    let mut lockers = merged.into_values().collect::<Vec<_>>();
    lockers.sort_by(|a, b| {
        b.is_system_process
            .cmp(&a.is_system_process)
            .then_with(|| a.pid.cmp(&b.pid))
    });
    Ok(lockers)
}

fn scan_lockers(path: &Path, language_index: i32) -> Result<Vec<LockerInfo>, ScanError> {
    use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
    use windows_sys::Win32::System::RestartManager::{
        CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
        RmStartSession,
    };

    let is_system_file = is_windows_system_path(path);
    let path_text = path.to_string_lossy().to_string();
    let wide_path = path_text
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();

    let mut session_handle: u32 = 0;
    let mut session_key = [0u16; (CCH_RM_SESSION_KEY + 1) as usize];
    let start_ret = unsafe { RmStartSession(&mut session_handle, 0, session_key.as_mut_ptr()) };
    if start_ret != 0 {
        return Err(ScanError::Start(start_ret));
    }

    let result = (|| {
        let file_ptrs = [wide_path.as_ptr()];
        let register_ret = unsafe {
            RmRegisterResources(
                session_handle,
                file_ptrs.len() as u32,
                file_ptrs.as_ptr(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
            )
        };
        if register_ret != 0 {
            return Err(ScanError::Register(register_ret));
        }

        let mut proc_info_needed = 0u32;
        let mut proc_info_count = 0u32;
        let mut reboot_reasons = 0u32;

        let mut get_ret = unsafe {
            RmGetList(
                session_handle,
                &mut proc_info_needed,
                &mut proc_info_count,
                std::ptr::null_mut(),
                &mut reboot_reasons,
            )
        };

        if get_ret != 0 && get_ret != ERROR_MORE_DATA {
            return Err(ScanError::GetList(get_ret));
        }

        if proc_info_needed == 0 {
            return Ok(Vec::new());
        }

        let mut proc_info =
            vec![unsafe { std::mem::zeroed::<RM_PROCESS_INFO>() }; proc_info_needed as usize];
        proc_info_count = proc_info_needed;
        get_ret = unsafe {
            RmGetList(
                session_handle,
                &mut proc_info_needed,
                &mut proc_info_count,
                proc_info.as_mut_ptr(),
                &mut reboot_reasons,
            )
        };

        if get_ret != 0 {
            return Err(ScanError::GetList(get_ret));
        }

        let mut lockers = Vec::with_capacity(proc_info_count as usize);
        for item in proc_info.into_iter().take(proc_info_count as usize) {
            let pid = item.Process.dwProcessId;
            let app_name = utf16_to_string(&item.strAppName);
            let process_name = if app_name.trim().is_empty() {
                format!("PID {}", pid)
            } else {
                app_name
            };
            let system_process = is_system_process(pid, &process_name);
            lockers.push(LockerInfo {
                process_name,
                pid,
                is_system_process: system_process,
                is_system_file,
                note: t(language_index, "unlock.note.locking"),
            });
        }

        lockers.sort_by(|a, b| {
            b.is_system_process
                .cmp(&a.is_system_process)
                .then_with(|| a.pid.cmp(&b.pid))
        });

        Ok(lockers)
    })();

    unsafe {
        RmEndSession(session_handle);
    }

    result
}

fn terminate_process(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return Err(format!("OpenProcess failed for PID {}", pid));
    }

    let ok = unsafe { TerminateProcess(handle, 1) };
    unsafe {
        CloseHandle(handle);
    }

    if ok == 0 {
        Err(format!("TerminateProcess failed for PID {}", pid))
    } else {
        Ok(())
    }
}

fn validate_target_path(target: &str, language_index: i32) -> Result<PathBuf, String> {
    let path = PathBuf::from(target.trim());
    if path.as_os_str().is_empty() {
        return Err(t(language_index, "unlock.msg.input_required"));
    }
    if !path.exists() {
        return Err(t(language_index, "unlock.msg.path_not_exists"));
    }
    if !path.is_file() && !path.is_dir() {
        return Err(t(language_index, "unlock.msg.path_type_unsupported"));
    }

    Ok(path)
}

fn perform_scan(
    ui: &MainWindow,
    target: &str,
    unlock_state: &Rc<RefCell<Option<UnlockState>>>,
) -> Option<Vec<LockerInfo>> {
    let language_index = ui.get_language_index();
    let path_not_exists_msg = t(language_index, "unlock.msg.path_not_exists");
    let path = match validate_target_path(target, language_index) {
        Ok(path) => path,
        Err(err) => {
            if err == path_not_exists_msg {
                ui.set_unlock_preview_text(err.clone().into());
                ui.set_unlock_preview_rows(ModelRc::new(VecModel::from(
                    Vec::<UnlockPreviewRow>::new(),
                )));
                *unlock_state.borrow_mut() = None;
                append_unlock_status_log(ui, "ERROR", &err);
                return None;
            }
            ui.set_unlock_preview_text(err.clone().into());
            ui.set_unlock_preview_rows(ModelRc::new(
                VecModel::from(Vec::<UnlockPreviewRow>::new()),
            ));
            *unlock_state.borrow_mut() = None;
            append_unlock_status_log(ui, "ERROR", &err);
            return None;
        }
    };

    match scan_target_lockers(&path, language_index) {
        Ok(lockers) => {
            ui.set_unlock_preview_text("".into());
            *unlock_state.borrow_mut() = Some(UnlockState {
                target_key: target.trim().to_string(),
                lockers: lockers.clone(),
                excluded_indices: HashSet::new(),
            });
            if let Some(state) = unlock_state.borrow().as_ref() {
                apply_unlock_exclusions(ui, state);
            }

            if lockers.is_empty() {
                append_unlock_status_log(ui, "INFO", &t(language_index, "unlock.msg.no_lockers"));
            } else {
                let count = lockers.len().to_string();
                append_unlock_status_log(
                    ui,
                    "INFO",
                    &tf(
                        language_index,
                        "unlock.msg.scan_success",
                        &[("count", &count)],
                    ),
                );
            }

            if is_windows_system_path(&path) {
                append_unlock_status_log(
                    ui,
                    "ERROR",
                    &t(language_index, "unlock.msg.system_file_blocked"),
                );
            } else if lockers.iter().any(|x| x.is_system_process) {
                append_unlock_status_log(
                    ui,
                    "WARN",
                    &t(language_index, "unlock.msg.system_process_warning"),
                );
            }

            Some(lockers)
        }
        Err(err) => {
            ui.set_unlock_preview_text(err.clone().into());
            ui.set_unlock_preview_rows(ModelRc::new(
                VecModel::from(Vec::<UnlockPreviewRow>::new()),
            ));
            *unlock_state.borrow_mut() = None;
            append_unlock_status_log(ui, "ERROR", &err);
            None
        }
    }
}

pub fn setup_unlock_handlers(ui: &MainWindow) {
    let latest_unlock_state: Rc<RefCell<Option<UnlockState>>> = Rc::new(RefCell::new(None));

    ui.set_unlock_status_text("".into());
    ui.set_unlock_preview_text("".into());
    ui.set_unlock_preview_rows(ModelRc::new(VecModel::from(Vec::<UnlockPreviewRow>::new())));
    append_unlock_status_log(ui, "INFO", &t(ui.get_language_index(), "unlock.msg.ready"));

    {
        let ui_handle = ui.as_weak();
        let unlock_state = Rc::clone(&latest_unlock_state);
        ui.on_unlock_scan_request(move |target| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let _ = perform_scan(&ui, target.as_str(), &unlock_state);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let unlock_state = Rc::clone(&latest_unlock_state);
        ui.on_unlock_remove_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let mut borrowed = unlock_state.borrow_mut();
            let Some(state) = borrowed.as_mut() else {
                return;
            };

            let Some(removed_name) = state.exclude_visible_row(index as usize) else {
                return;
            };
            apply_unlock_exclusions(&ui, state);
            append_unlock_status_log(
                &ui,
                "INFO",
                &tf(
                    ui.get_language_index(),
                    "unlock.msg.row_removed",
                    &[("name", &removed_name)],
                ),
            );
        });
    }

    {
        let ui_handle = ui.as_weak();
        let unlock_state = Rc::clone(&latest_unlock_state);
        ui.on_unlock_release_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let language_index = ui.get_language_index();
            let (target_key, lockers) = {
                let borrowed = unlock_state.borrow();
                let Some(state) = borrowed.as_ref() else {
                    append_unlock_status_log(
                        &ui,
                        "ERROR",
                        &t(language_index, "unlock.msg.scan_first"),
                    );
                    return;
                };

                (state.target_key.clone(), state.filtered_lockers())
            };

            let path = match validate_target_path(&target_key, language_index) {
                Ok(path) => path,
                Err(err) => {
                    append_unlock_status_log(&ui, "ERROR", &err);
                    return;
                }
            };

            if is_windows_system_path(&path) {
                append_unlock_status_log(
                    &ui,
                    "ERROR",
                    &t(language_index, "unlock.msg.system_file_blocked"),
                );
                return;
            }

            if lockers.is_empty() {
                append_unlock_status_log(&ui, "INFO", &t(language_index, "unlock.msg.no_lockers"));
                return;
            }

            if lockers.iter().any(|x| x.is_system_process) {
                append_unlock_status_log(
                    &ui,
                    "WARN",
                    &t(language_index, "unlock.msg.system_process_warning"),
                );
                append_unlock_status_log(
                    &ui,
                    "ERROR",
                    &t(language_index, "unlock.msg.system_process_blocked"),
                );
                return;
            }

            let mut ok_count = 0usize;
            let mut fail_count = 0usize;
            for locker in lockers {
                match terminate_process(locker.pid) {
                    Ok(()) => ok_count += 1,
                    Err(err) => {
                        fail_count += 1;
                        append_unlock_status_log(
                            &ui,
                            "ERROR",
                            &tf(
                                language_index,
                                "unlock.msg.release_item_failed",
                                &[("pid", &locker.pid.to_string()), ("error", &err)],
                            ),
                        );
                    }
                }
            }

            append_unlock_status_log(
                &ui,
                "INFO",
                &tf(
                    language_index,
                    "unlock.msg.release_result",
                    &[
                        ("ok", &ok_count.to_string()),
                        ("failed", &fail_count.to_string()),
                    ],
                ),
            );

            let _ = perform_scan(&ui, &target_key, &unlock_state);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let unlock_state = Rc::clone(&latest_unlock_state);
        ui.on_unlock_clear_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let mut borrowed = unlock_state.borrow_mut();
            if let Some(state) = borrowed.as_mut() {
                state.exclude_all_rows();
                apply_unlock_exclusions(&ui, state);
            } else {
                ui.set_unlock_preview_rows(ModelRc::new(VecModel::from(
                    Vec::<UnlockPreviewRow>::new(),
                )));
            }
            ui.set_unlock_preview_text("".into());
            append_unlock_status_log(
                &ui,
                "INFO",
                &t(ui.get_language_index(), "unlock.msg.cleared"),
            );
        });
    }
}
