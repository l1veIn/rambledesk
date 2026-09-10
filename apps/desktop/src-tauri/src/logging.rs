use std::{fs, io::Write, path::PathBuf, sync::Arc};

use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::diagnostics::control::{self, DiagnosticsControl, RecordingGuard};

const LOG_FILE_PREFIX: &str = "rambledesk.log";
const LOG_FILES_TO_KEEP: usize = 7;

pub(crate) fn init() {
    // Load the persisted choice before startup events or the tracing subscriber.
    let control = control::global().ok().cloned();
    install_panic_hook();
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let Ok(log_dir) = log_directory().and_then(|path| {
        fs::create_dir_all(&path).map_err(|error| error.to_string())?;
        Ok(path)
    }) else {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(recording_filter(control.clone()))
            .with(tracing_subscriber::fmt::layer().with_target(false))
            .try_init();
        return;
    };

    // `RollingFileAppender::new` panics internally when it cannot create or
    // open the log file; at startup that panic would abort the entire process.
    // Catch it so an unwritable data directory only loses file logging.
    let appender =
        std::panic::catch_unwind(|| tracing_appender::rolling::daily(&log_dir, LOG_FILE_PREFIX))
            .ok();
    let Some(file_appender) = appender else {
        write_init_error(&log_dir, "rolling file appender creation failed\n");
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(recording_filter(control.clone()))
            .with(tracing_subscriber::fmt::layer().with_target(false))
            .try_init();
        return;
    };
    // Include today's newly created file in the retention count.
    prune_old_logs(&log_dir);
    let console_layer = tracing_subscriber::fmt::layer().with_target(false);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(false)
        .with_writer(DiagnosticLogAppender {
            inner: file_appender,
            control: control.clone(),
        });
    if let Err(error) = tracing_subscriber::registry()
        .with(filter)
        .with(recording_filter(control.clone()))
        .with(console_layer)
        .with(file_layer)
        .try_init()
    {
        let fallback = format!("persistent logging initialization failed: {error}\n");
        write_init_error(&log_dir, &fallback);
        return;
    }
    tracing::info!(directory = %log_dir.display(), "persistent logging initialized");
}

fn recording_filter<S>(
    control: Option<Arc<DiagnosticsControl>>,
) -> impl tracing_subscriber::Layer<S>
where
    S: tracing::Subscriber,
{
    // The preference changes at runtime. A metadata-only FilterFn caches a
    // callsite as disabled, which would keep it silent after recording resumes.
    tracing_subscriber::filter::dynamic_filter_fn(move |_, _| {
        control
            .as_ref()
            .and_then(|control| control.recording_guard())
            .is_some()
    })
}

fn write_init_error(directory: &std::path::Path, message: &str) {
    if let Some(_recording) = control::recording_guard() {
        let _ = fs::write(directory.join("rambledesk.log.init-error"), message);
    }
}

struct DiagnosticLogAppender {
    inner: tracing_appender::rolling::RollingFileAppender,
    control: Option<Arc<DiagnosticsControl>>,
}

struct DiagnosticLogWriter<'a> {
    _recording: Option<RecordingGuard<'a>>,
    inner: Option<tracing_appender::rolling::RollingWriter<'a>>,
}

impl<'a> MakeWriter<'a> for DiagnosticLogAppender {
    type Writer = DiagnosticLogWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        let recording = self
            .control
            .as_ref()
            .and_then(|control| control.recording_guard());
        let inner = recording.as_ref().map(|_| self.inner.make_writer());
        DiagnosticLogWriter {
            _recording: recording,
            inner,
        }
    }
}

impl Write for DiagnosticLogWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            Some(writer) => writer.write(bytes),
            None => Ok(bytes.len()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.inner {
            Some(writer) => writer.flush(),
            None => Ok(()),
        }
    }
}

pub(crate) fn directory_hint() -> String {
    log_directory()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "RambleDesk application data directory".to_owned())
}

fn install_panic_hook() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info.location();
            let file = location
                .and_then(|location| location.file().rsplit(['/', '\\']).next())
                .unwrap_or("unknown");
            tracing::error!(
                source_file = file,
                line = location.map_or(0, |location| location.line()),
                column = location.map_or(0, |location| location.column()),
                "native panic (payload omitted)"
            );
            previous(info);
        }));
    });
}

pub(crate) fn directory() -> Result<PathBuf, String> {
    log_directory()
}

pub(crate) fn frontend_error(context: &str, _message: &str) {
    tracing::error!(
        context = frontend_context(context),
        "frontend error details omitted; use structured client diagnostics"
    );
}

pub(crate) fn show_fatal_startup_error(error: &(dyn std::error::Error + 'static)) {
    let message = crate::startup::fatal_startup_message(error);
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

fn frontend_context(value: &str) -> &str {
    match value {
        "window" | "unhandledrejection" | "updater" | "main" | "bootstrap" => value,
        _ => "unknown",
    }
}

#[cfg(target_os = "windows")]
fn show_native_error(message: &str) {
    use windows::{
        Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
        core::PCWSTR,
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

#[cfg(target_os = "macos")]
fn show_native_error(message: &str) {
    // Tauri could not build, so its event loop and dialog plugin are unavailable.
    // Pass the message as an argument, never interpolate it into AppleScript.
    let shown = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg("on run argv\n display alert \"RambleDesk 启动失败\" message (item 1 of argv) as critical buttons {\"退出\"} default button \"退出\"\nend run")
        .arg("--")
        .arg(message)
        .status()
        .is_ok_and(|status| status.success());
    if !shown {
        eprintln!("{message}");
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn show_native_error(message: &str) {
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_error_accepts_only_static_context() {
        assert_eq!(frontend_context("window"), "window");
        assert_eq!(frontend_context("user task or credential"), "unknown");
    }

    #[test]
    fn disabling_stops_file_logging_and_clear_keeps_the_active_writer_usable() {
        let directory = tempfile::tempdir().unwrap();
        let log_directory = directory.path().join("logs");
        let control = Arc::new(DiagnosticsControl::load(directory.path().to_owned()));
        let appender = DiagnosticLogAppender {
            inner: tracing_appender::rolling::never(&log_directory, LOG_FILE_PREFIX),
            control: Some(control.clone()),
        };
        let path = log_directory.join(LOG_FILE_PREFIX);
        appender.make_writer().write_all(b"before\n").unwrap();
        control.set_enabled(false).unwrap();
        appender
            .make_writer()
            .write_all(b"must not be recorded\n")
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"before\n");
        control.clear().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"");
        control.set_enabled(true).unwrap();
        appender.make_writer().write_all(b"after\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"after\n");
    }

    #[test]
    fn clear_waits_for_the_active_log_record_before_truncating() {
        let directory = tempfile::tempdir().unwrap();
        let log_directory = directory.path().join("logs");
        let control = Arc::new(DiagnosticsControl::load(directory.path().to_owned()));
        let appender = DiagnosticLogAppender {
            inner: tracing_appender::rolling::never(&log_directory, LOG_FILE_PREFIX),
            control: Some(control.clone()),
        };
        let mut writer = appender.make_writer();
        let (started, start) = std::sync::mpsc::channel();
        let (completed, completion) = std::sync::mpsc::channel();
        let clearing = std::thread::spawn(move || {
            started.send(()).unwrap();
            completed.send(control.clear()).unwrap();
        });
        start.recv().unwrap();
        assert!(
            completion
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        writer.write_all(b"in-flight log record\n").unwrap();
        drop(writer);
        completion
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap();
        clearing.join().unwrap();
        assert_eq!(fs::read(log_directory.join(LOG_FILE_PREFIX)).unwrap(), b"");
    }

    #[test]
    fn same_tracing_callsite_obeys_each_recording_toggle() {
        fn emit_probe(phase: &str) {
            tracing::info!(phase, "diagnostic toggle probe");
        }
        let directory = tempfile::tempdir().unwrap();
        let log_directory = directory.path().join("logs");
        let control = Arc::new(DiagnosticsControl::load(directory.path().to_owned()));
        control.set_enabled(false).unwrap();
        let appender = DiagnosticLogAppender {
            inner: tracing_appender::rolling::never(&log_directory, LOG_FILE_PREFIX),
            control: Some(control.clone()),
        };
        let subscriber = tracing_subscriber::registry()
            .with(recording_filter(Some(control.clone())))
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(appender),
            );
        tracing::subscriber::with_default(subscriber, || {
            emit_probe("off-initial");
            control.set_enabled(true).unwrap();
            emit_probe("on-first");
            control.set_enabled(false).unwrap();
            emit_probe("off-again");
            control.set_enabled(true).unwrap();
            emit_probe("on-again");
        });
        let text = fs::read_to_string(log_directory.join(LOG_FILE_PREFIX)).unwrap();
        assert_eq!(text.matches("diagnostic toggle probe").count(), 2);
        assert!(text.contains("on-first") && text.contains("on-again"));
        assert!(!text.contains("off-"));
    }
}
