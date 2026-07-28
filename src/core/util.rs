use crate::public::lang::sanitize_ui_text;
use windows_sys::Win32::Foundation::SYSTEMTIME;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

const MAX_LOG_LINES: usize = 100;

fn current_time_prefix() -> String {
    let mut local_time = SYSTEMTIME::default();
    unsafe {
        GetLocalTime(&mut local_time);
    }

    format!(
        "{:02}:{:02}:{:02}",
        local_time.wHour, local_time.wMinute, local_time.wSecond
    )
}

pub fn append_log_line(current: &str, message: &str) -> String {
    let mut lines = if current.trim().is_empty() {
        Vec::new()
    } else {
        current.lines().map(|s| s.to_string()).collect::<Vec<_>>()
    };

    lines.push(format!(
        "[{}] {}",
        current_time_prefix(),
        sanitize_ui_text(message)
    ));

    if lines.len() > MAX_LOG_LINES {
        let drop_count = lines.len() - MAX_LOG_LINES;
        lines.drain(0..drop_count);
    }

    lines.join("\n")
}
