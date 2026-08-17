use std::{fs, path::PathBuf};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const LOG_FILE_PREFIX: &str = "rambledesk.log";
const LOG_FILES_TO_KEEP: usize = 7;

pub(crate) fn init() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let Ok(log_dir) = log_directory().and_then(|path| {
        fs::create_dir_all(&path).map_err(|error| error.to_string())?;
        Ok(path)
    }) else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .try_init();
        return;
    };

    prune_old_logs(&log_dir);
    let file_appender = tracing_appender::rolling::daily(&log_dir, LOG_FILE_PREFIX);
    let console_layer = tracing_subscriber::fmt::layer().with_target(false);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(false)
        .with_writer(file_appender);
    if let Err(error) = tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .try_init()
    {
        let fallback = format!("persistent logging initialization failed: {error}\n");
        let _ = fs::write(log_dir.join("rambledesk.log.init-error"), fallback);
        return;
    }
    tracing::info!(directory = %log_dir.display(), "persistent logging initialized");
}

pub(crate) fn directory_hint() -> String {
    log_directory()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "RambleDesk application data directory".to_owned())
}

pub(crate) fn directory() -> Result<PathBuf, String> {
    log_directory()
}

pub(crate) fn frontend_error(context: &str, message: &str) {
    tracing::error!(
        context = %sanitize(context, 128),
        message = %sanitize(message, 4_096),
        "frontend error"
    );
}

pub(crate) fn show_fatal_startup_error(error: &str) {
    let message = format!(
        "RambleDesk 无法启动。\n\n{error}\n\n诊断日志目录：\n{}",
        directory_hint()
    );
    tracing::error!(%error, "RambleDesk startup failed");
    show_native_error(&message);
}

fn log_directory() -> Result<PathBuf, String> {
    rambledesk_storage::default_app_data_root()
        .map(|root| root.join("logs"))
        .map_err(|error| error.to_string())
}

fn prune_old_logs(directory: &PathBuf) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut logs = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(LOG_FILE_PREFIX)
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    logs.sort_by_key(|(modified, _)| *modified);
    let remove_count = logs.len().saturating_sub(LOG_FILES_TO_KEEP);
    for (_, path) in logs.into_iter().take(remove_count) {
        let _ = fs::remove_file(path);
    }
}

fn sanitize(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(max_chars)
        .collect()
}

#[cfg(target_os = "windows")]
fn show_native_error(message: &str) {
    use windows::{
        core::PCWSTR,
        Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK},
    };

    let title = "RambleDesk 启动失败"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let message = message
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_native_error(message: &str) {
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    #[test]
    fn sanitize_limits_and_removes_control_characters() {
        assert_eq!(sanitize("a\0b\nc", 4), "ab\nc");
    }
}
