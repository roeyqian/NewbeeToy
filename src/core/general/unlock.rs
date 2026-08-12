use std::collections::{HashMap, HashSet, VecDeque};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::core::util::append_log_line;
use crate::public::assets::lang::{sanitize_ui_text, t, tf};
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
static WINDOWS_DIR_PREFIX: OnceLock<String> = OnceLock::new();

#[repr(C)]
#[derive(Clone, Copy)]
struct SystemHandleEntry {
    object: usize,
    unique_process_id: usize,
    handle_value: usize,
    granted_access: u32,
    creator_back_trace_index: u16,
    object_type_index: u16,
    handle_attributes: u32,
    reserved: u32,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQuerySystemInformation(
        system_information_class: i32,
        system_information: *mut std::ffi::c_void,
        system_information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}

const SYSTEM_EXTENDED_HANDLE_INFORMATION: i32 = 64;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;

fn normalize_windows_path(path: &Path) -> String {
    normalize_windows_path_text(&path.to_string_lossy())
}

fn normalize_windows_path_text(path: &str) -> String {
    let path = path.replace('/', "\\");
    let path = path
        .strip_prefix("\\\\?\\UNC\\")
        .map(|rest| format!("\\\\{}", rest))
        .or_else(|| path.strip_prefix("\\\\?\\").map(str::to_string))
        .unwrap_or(path);
    trim_windows_path_tail(path.to_lowercase())
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

fn collect_directory_files(dir: &Path) -> (Vec<PathBuf>, bool) {
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

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    if err.kind() == ErrorKind::PermissionDenied {
                        has_permission_denied = true;
                    }
                    continue;
                }
            };
            // Do not traverse junctions or symbolic links. They can point outside the selected
            // directory (or form a loop), which would make an unrestricted scan unbounded.
            if file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            if file_type.is_dir() {
                queue.push_back(path);
                continue;
            }
            if file_type.is_file() {
                files.push(path);
            }
        }
    }

    (files, has_permission_denied)
}

fn scan_target_lockers(path: &Path, language_index: i32) -> Result<Vec<LockerInfo>, String> {
    let mut merged = HashMap::<u32, LockerInfo>::new();

    // Restart Manager cannot inspect directory resources. The handle-table scan also catches
    // handles held on the directory itself, as well as handles on files below it.
    for locker in scan_open_handle_lockers(path, language_index) {
        merge_lockers(&mut merged, locker);
    }

    if path.is_file() {
        for locker in scan_lockers(&[path.to_path_buf()], language_index)
            .map_err(|err| format_scan_error(language_index, err))?
        {
            merge_lockers(&mut merged, locker);
        }
        return Ok(sorted_lockers(merged));
    }

    let (files, read_permission_denied) = collect_directory_files(path);
    if files.is_empty() {
        return if read_permission_denied {
            Err(t(language_index, "unlock.msg.directory_access_denied"))
        } else {
            Ok(sorted_lockers(merged))
        };
    }

    match scan_lockers(&files, language_index) {
        Ok(lockers) => {
            for locker in lockers {
                merge_lockers(&mut merged, locker);
            }
        }
        Err(ScanError::Start(code) | ScanError::Register(code) | ScanError::GetList(code))
            if code == ACCESS_DENIED_CODE && read_permission_denied =>
        {
            return Err(t(language_index, "unlock.msg.directory_access_denied"));
        }
        Err(err) => return Err(format_scan_error(language_index, err)),
    }

    Ok(sorted_lockers(merged))
}

fn sorted_lockers(merged: HashMap<u32, LockerInfo>) -> Vec<LockerInfo> {
    let mut lockers = merged.into_values().collect::<Vec<_>>();
    lockers.sort_by(|a, b| {
        b.is_system_process
            .cmp(&a.is_system_process)
            .then_with(|| a.pid.cmp(&b.pid))
    });
    lockers
}

fn scan_lockers(paths: &[PathBuf], language_index: i32) -> Result<Vec<LockerInfo>, ScanError> {
    use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
    use windows_sys::Win32::System::RestartManager::{
        CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
        RmStartSession,
    };

    let is_system_file = paths.iter().any(|path| is_windows_system_path(path));
    let wide_paths = paths
        .iter()
        .map(|path| {
            path.to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        })
        .collect::<Vec<_>>();
    let file_ptrs = wide_paths
        .iter()
        .map(|path| path.as_ptr())
        .collect::<Vec<_>>();

    let mut session_handle: u32 = 0;
    let mut session_key = [0u16; (CCH_RM_SESSION_KEY + 1) as usize];
    let start_ret = unsafe { RmStartSession(&mut session_handle, 0, session_key.as_mut_ptr()) };
    if start_ret != 0 {
        return Err(ScanError::Start(start_ret));
    }

    let result = (|| {
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

fn scan_open_handle_lockers(path: &Path, language_index: i32) -> Vec<LockerInfo> {
    use windows_sys::Win32::Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle};
    use windows_sys::Win32::Storage::FileSystem::GetFileType;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE,
    };

    let Some(buffer) = query_system_handle_buffer() else {
        return Vec::new();
    };
    let header_size = std::mem::size_of::<usize>() * 2;
    if buffer.len() < header_size {
        return Vec::new();
    }

    let count = unsafe { (buffer.as_ptr() as *const usize).read_unaligned() };
    let entry_size = std::mem::size_of::<SystemHandleEntry>();
    let count = count.min((buffer.len() - header_size) / entry_size);
    let target = normalize_windows_path(path);
    let target_prefix = format!("{}\\", target);
    let target_is_directory = path.is_dir();
    let mut processes = HashMap::new();
    let mut merged = HashMap::new();
    let current_process = unsafe { GetCurrentProcess() };

    for index in 0..count {
        let offset = header_size + index * entry_size;
        let entry =
            unsafe { (buffer.as_ptr().add(offset) as *const SystemHandleEntry).read_unaligned() };
        let pid = entry.unique_process_id as u32;
        if pid == 0 || pid == 4 || pid == std::process::id() {
            continue;
        }

        let process = *processes
            .entry(pid)
            .or_insert_with(|| unsafe { OpenProcess(PROCESS_DUP_HANDLE, 0, pid) });
        if process.is_null() {
            continue;
        }

        let mut duplicated = std::ptr::null_mut();
        if unsafe {
            DuplicateHandle(
                process,
                entry.handle_value as _,
                current_process,
                &mut duplicated,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        } == 0
        {
            continue;
        }

        let handle_path = final_path_for_handle(duplicated, unsafe { GetFileType(duplicated) });
        unsafe {
            CloseHandle(duplicated);
        }
        let Some(handle_path) = handle_path else {
            continue;
        };
        if !(handle_path == target
            || (target_is_directory && handle_path.starts_with(&target_prefix)))
        {
            continue;
        }

        let process_name = query_process_path(pid)
            .and_then(|process_path| {
                Path::new(&process_path)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| format!("PID {}", pid));
        merge_lockers(
            &mut merged,
            LockerInfo {
                process_name: process_name.clone(),
                pid,
                is_system_process: is_system_process(pid, &process_name),
                is_system_file: is_windows_system_path(path),
                note: t(language_index, "unlock.note.locking"),
            },
        );
    }

    for process in processes.into_values() {
        if !process.is_null() {
            unsafe {
                CloseHandle(process);
            }
        }
    }
    sorted_lockers(merged)
}

fn query_system_handle_buffer() -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let mut required = 0u32;
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_EXTENDED_HANDLE_INFORMATION,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut required,
            )
        };
        if status == 0 {
            return Some(buffer);
        }
        if status != STATUS_INFO_LENGTH_MISMATCH || buffer.len() >= 64 * 1024 * 1024 {
            return None;
        }
        buffer.resize((required as usize).max(buffer.len() * 2), 0);
    }
}

fn final_path_for_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
    file_type: windows_sys::Win32::Storage::FileSystem::FILE_TYPE,
) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NAME_NORMALIZED, FILE_TYPE_DISK, GetFinalPathNameByHandleW,
    };

    if file_type != FILE_TYPE_DISK {
        return None;
    }
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            handle,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            FILE_NAME_NORMALIZED,
        )
    };
    if length == 0 || length as usize >= buffer.len() {
        return None;
    }
    Some(normalize_windows_path_text(&String::from_utf16_lossy(
        &buffer[..length as usize],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn detects_a_process_holding_the_target_directory() {
        let dir = std::env::temp_dir().join(format!("newbee-unlock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temporary directory");
        let mut child = Command::new("cmd")
            .args(["/C", "ping 127.0.0.1 -n 10 > nul"])
            .current_dir(&dir)
            .spawn()
            .expect("start process with the directory as its working directory");

        std::thread::sleep(Duration::from_millis(150));
        let lockers = scan_open_handle_lockers(&dir, 0);
        let found = lockers.iter().any(|locker| locker.pid == child.id());

        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            found,
            "the child process's directory handle was not detected"
        );
    }
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

fn apply_scan_result(
    ui: &MainWindow,
    target_key: String,
    path: PathBuf,
    result: Result<Vec<LockerInfo>, String>,
    unlock_state: &std::sync::Arc<std::sync::Mutex<Option<UnlockState>>>,
) {
    let language_index = ui.get_language_index();
    match result {
        Ok(lockers) => {
            ui.set_unlock_preview_text("".into());
            let Ok(mut state) = unlock_state.lock() else {
                return;
            };
            *state = Some(UnlockState {
                target_key,
                lockers: lockers.clone(),
                excluded_indices: HashSet::new(),
            });
            if let Some(state) = state.as_ref() {
                apply_unlock_exclusions(ui, state);
            }
            drop(state);

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
        }
        Err(err) => {
            ui.set_unlock_preview_text(err.clone().into());
            ui.set_unlock_preview_rows(ModelRc::new(
                VecModel::from(Vec::<UnlockPreviewRow>::new()),
            ));
            if let Ok(mut state) = unlock_state.lock() {
                *state = None;
            }
            append_unlock_status_log(ui, "ERROR", &err);
        }
    }
}

fn start_scan(
    ui: &MainWindow,
    target: &str,
    unlock_state: &std::sync::Arc<std::sync::Mutex<Option<UnlockState>>>,
) {
    let language_index = ui.get_language_index();
    let path = match validate_target_path(target, language_index) {
        Ok(path) => path,
        Err(err) => {
            ui.set_unlock_preview_text(err.clone().into());
            ui.set_unlock_preview_rows(ModelRc::new(
                VecModel::from(Vec::<UnlockPreviewRow>::new()),
            ));
            if let Ok(mut state) = unlock_state.lock() {
                *state = None;
            }
            append_unlock_status_log(ui, "ERROR", &err);
            return;
        }
    };

    let target_key = target.trim().to_string();
    if let Ok(mut state) = unlock_state.lock() {
        *state = None;
    }
    ui.set_unlock_preview_text("".into());
    ui.set_unlock_preview_rows(ModelRc::new(VecModel::from(Vec::<UnlockPreviewRow>::new())));
    ui.set_unlock_scanning(true);
    append_unlock_status_log(ui, "INFO", &t(language_index, "unlock.msg.scanning"));

    let ui_handle = ui.as_weak();
    let unlock_state = std::sync::Arc::clone(unlock_state);
    std::thread::spawn(move || {
        let result = scan_target_lockers(&path, language_index);
        let _ = ui_handle.upgrade_in_event_loop(move |ui| {
            ui.set_unlock_scanning(false);
            apply_scan_result(&ui, target_key, path, result, &unlock_state);
        });
    });
}

pub fn setup_unlock_handlers(ui: &MainWindow) {
    let latest_unlock_state = std::sync::Arc::new(std::sync::Mutex::new(None));

    ui.set_unlock_status_text("".into());
    ui.set_unlock_preview_text("".into());
    ui.set_unlock_scanning(false);
    ui.set_unlock_preview_rows(ModelRc::new(VecModel::from(Vec::<UnlockPreviewRow>::new())));
    append_unlock_status_log(ui, "INFO", &t(ui.get_language_index(), "unlock.msg.ready"));

    {
        let ui_handle = ui.as_weak();
        let unlock_state = std::sync::Arc::clone(&latest_unlock_state);
        ui.on_unlock_scan_request(move |target| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            start_scan(&ui, target.as_str(), &unlock_state);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let unlock_state = std::sync::Arc::clone(&latest_unlock_state);
        ui.on_unlock_remove_row_request(move |index| {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let Ok(mut borrowed) = unlock_state.lock() else {
                return;
            };
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
        let unlock_state = std::sync::Arc::clone(&latest_unlock_state);
        ui.on_unlock_release_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let language_index = ui.get_language_index();
            let (target_key, lockers) = {
                let Ok(borrowed) = unlock_state.lock() else {
                    return;
                };
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

            start_scan(&ui, &target_key, &unlock_state);
        });
    }

    {
        let ui_handle = ui.as_weak();
        let unlock_state = std::sync::Arc::clone(&latest_unlock_state);
        ui.on_unlock_clear_request(move || {
            let Some(ui) = ui_handle.upgrade() else {
                return;
            };

            let Ok(mut borrowed) = unlock_state.lock() else {
                return;
            };
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
