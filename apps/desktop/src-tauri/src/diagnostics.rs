//! Privacy-preserving diagnostic package export.
//!
//! The zip contains environment, runtime, model, request metadata, usage
//! events, and redacted logs. It never copies drafts, feedback markdown,
//! attachments, tokens, API keys, or request titles.

mod events;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rambledesk_core::{FeedbackApplication, FeedbackStatus, ListFeedbackRequestsInput};
use rambledesk_speech::model::list_models;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use zip::write::SimpleFileOptions;

use crate::WorkbenchState;
use crate::config::load_storage_preferences;
use crate::logging;
use crate::macos_permissions::list_macos_permissions;

pub(crate) use events::record as record_event;

const PACKAGE_SCHEMA_VERSION: u32 = 1;
const MAX_LOG_FILE_BYTES: u64 = 1024 * 1024;
const MAX_REQUESTS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum DiagnosticScope {
    #[serde(rename = "last_24_hours")]
    LastTwentyFourHours,
    #[serde(rename = "last_7_days", alias = "last_seven_days")]
    LastSevenDays,
    #[serde(rename = "all")]
    All,
}

impl DiagnosticScope {
    fn as_label(self) -> &'static str {
        match self {
            Self::LastTwentyFourHours => "last_24_hours",
            Self::LastSevenDays => "last_7_days",
            Self::All => "all",
        }
    }

    fn lookback(self) -> Option<Duration> {
        match self {
            Self::LastTwentyFourHours => Some(Duration::from_secs(24 * 3600)),
            Self::LastSevenDays => Some(Duration::from_secs(7 * 24 * 3600)),
            Self::All => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticExportResult {
    pub report_id: String,
    pub path: String,
    pub scope: String,
    pub event_count: usize,
    pub request_count: usize,
    pub log_file_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest<'a> {
    schema_version: u32,
    report_id: &'a str,
    generated_at: String,
    app_version: &'a str,
    scope: &'a str,
    contains_feedback_text: bool,
    contains_attachments: bool,
    contains_tokens: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Environment {
    os_name: Option<String>,
    os_version: Option<String>,
    kernel_version: Option<String>,
    architecture: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeSnapshot {
    app_session_id: String,
    data_storage_customized: bool,
    library_root: String,
    local_server_loopback: bool,
    speech_session_active: bool,
    macos_permissions: Vec<PermissionSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PermissionSnapshot {
    id: String,
    status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelSnapshot {
    id: String,
    engine_id: String,
    size_bytes: u64,
    installed: bool,
    streaming: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AdapterSnapshot {
    id: String,
    installed: bool,
    configured: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestMetadata {
    request_id: String,
    host_id: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageSummary {
    request_count: usize,
    by_status: serde_json::Value,
    by_host: serde_json::Value,
}

#[tauri::command]
pub async fn export_diagnostics(
    scope: DiagnosticScope,
    path: String,
    app: AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<DiagnosticExportResult, String> {
    let destination = with_zip_extension(Path::new(&path));
    let app_version = app.package_info().version.to_string();
    let report_id = format!("RD-{}", uuid::Uuid::now_v7());
    let generated_at = events::utc_now_rfc3339();
    let since = match scope {
        DiagnosticScope::LastTwentyFourHours => Some(hours_ago(24)),
        DiagnosticScope::LastSevenDays => Some(hours_ago(7 * 24)),
        DiagnosticScope::All => None,
    };

    let environment = Environment {
        os_name: sysinfo::System::name(),
        os_version: sysinfo::System::os_version(),
        kernel_version: sysinfo::System::kernel_version(),
        architecture: std::env::consts::ARCH,
    };
    let runtime = runtime_snapshot(&state).await;
    let models: Vec<ModelSnapshot> = list_models(&state.library_root())
        .into_iter()
        .map(|model| ModelSnapshot {
            id: model.id.to_owned(),
            engine_id: model.engine_id.to_owned(),
            size_bytes: model.size_bytes,
            installed: model.installed,
            streaming: model.streaming,
        })
        .collect();
    let adapters: Vec<AdapterSnapshot> = app
        .path()
        .home_dir()
        .map(|home| rambledesk_mcp::detect_hosts(&home))
        .unwrap_or_default()
        .into_iter()
        .map(|host| AdapterSnapshot {
            id: host.id.to_owned(),
            installed: host.installed,
            configured: host.configured,
        })
        .collect();
    let requests = request_metadata(&state.application, since.as_deref()).await?;
    let events = events::list_since(&events::events_path(), since.as_deref());
    let logs = collect_logs(since.as_deref(), scope.lookback())?;
    let summary = usage_summary(&requests);
    let manifest = Manifest {
        schema_version: PACKAGE_SCHEMA_VERSION,
        report_id: &report_id,
        generated_at,
        app_version: &app_version,
        scope: scope.as_label(),
        contains_feedback_text: false,
        contains_attachments: false,
        contains_tokens: false,
    };

    let mut entries = vec![
        (
            "manifest.json".to_owned(),
            pretty_json(&manifest, "序列化诊断包清单")?,
        ),
        (
            "environment.json".to_owned(),
            pretty_json(&environment, "序列化环境信息")?,
        ),
        (
            "runtime.json".to_owned(),
            pretty_json(&runtime, "序列化运行状态")?,
        ),
        (
            "models.json".to_owned(),
            pretty_json(&models, "序列化语音模型状态")?,
        ),
        (
            "adapters.json".to_owned(),
            pretty_json(&adapters, "序列化适配器状态")?,
        ),
        (
            "usage-summary.json".to_owned(),
            pretty_json(&summary, "序列化使用摘要")?,
        ),
        (
            "requests.json".to_owned(),
            pretty_json(&requests, "序列化请求元数据")?,
        ),
        (
            "events.csv".to_owned(),
            events::to_csv(&events, &app_version),
        ),
        (
            "README.txt".to_owned(),
            package_readme(&report_id, scope.as_label()),
        ),
    ];
    for (name, contents) in logs {
        entries.push((format!("logs/{name}"), contents));
    }

    write_zip(&destination, &entries)?;
    Ok(DiagnosticExportResult {
        report_id,
        path: crate::open_attachment::display_os_path(&destination),
        scope: scope.as_label().to_owned(),
        event_count: events.len(),
        request_count: requests.len(),
        log_file_count: entries
            .iter()
            .filter(|(name, _)| name.starts_with("logs/"))
            .count(),
    })
}

#[tauri::command]
pub fn record_diagnostic_event(activity: String, case_id: Option<String>) -> Result<(), String> {
    match activity.as_str() {
        "ramble_started" | "ramble_stopped" => {
            events::record(&activity, case_id.as_deref(), None, Some("ok"), None, None);
            Ok(())
        }
        other => Err(format!("未知的诊断事件：{other}")),
    }
}

async fn runtime_snapshot(state: &WorkbenchState) -> RuntimeSnapshot {
    let customized = load_storage_preferences()
        .ok()
        .and_then(|preferences| preferences.data_storage_path)
        .is_some();
    let speech_session_active = state.speech_session.lock().await.is_some();
    let library_root = state.library_root();
    RuntimeSnapshot {
        app_session_id: events::app_session_id().to_owned(),
        data_storage_customized: customized,
        library_root: redact_home(&library_root.display().to_string()),
        local_server_loopback: true,
        speech_session_active,
        macos_permissions: list_macos_permissions()
            .into_iter()
            .map(|permission| PermissionSnapshot {
                id: permission.id,
                status: serde_json::to_value(permission.status)
                    .ok()
                    .and_then(|value| value.as_str().map(ToOwned::to_owned))
                    .unwrap_or_else(|| "unknown".to_owned()),
            })
            .collect(),
    }
}

async fn request_metadata(
    application: &FeedbackApplication,
    since: Option<&str>,
) -> Result<Vec<RequestMetadata>, String> {
    let mut collected = Vec::new();
    let mut cursor = None;
    loop {
        let page = application
            .list_feedback_requests(ListFeedbackRequestsInput {
                host_id: None,
                host_session_id: None,
                status: Some(vec![
                    FeedbackStatus::Waiting,
                    FeedbackStatus::InProgress,
                    FeedbackStatus::Completed,
                    FeedbackStatus::Cancelled,
                ]),
                archived: None,
                search: None,
                limit: Some(100),
                cursor,
            })
            .await
            .map_err(|error| error.to_string())?;
        for request in page.requests {
            if since.is_some_and(|bound| request.created_at.as_str() < bound) {
                continue;
            }
            collected.push(RequestMetadata {
                request_id: request.request_id,
                host_id: request.host_id,
                status: request.status.as_str().to_owned(),
                created_at: request.created_at,
                updated_at: request.updated_at,
            });
            if collected.len() >= MAX_REQUESTS {
                return Ok(collected);
            }
        }
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    Ok(collected)
}

fn usage_summary(requests: &[RequestMetadata]) -> UsageSummary {
    let mut by_status = serde_json::Map::new();
    let mut by_host = serde_json::Map::new();
    for request in requests {
        increment(&mut by_status, &request.status);
        increment(&mut by_host, &request.host_id);
    }
    UsageSummary {
        request_count: requests.len(),
        by_status: serde_json::Value::Object(by_status),
        by_host: serde_json::Value::Object(by_host),
    }
}

fn increment(map: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    let count = map
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    map.insert(key.to_owned(), serde_json::json!(count + 1));
}

fn collect_logs(
    since: Option<&str>,
    lookback: Option<Duration>,
) -> Result<Vec<(String, String)>, String> {
    let directory = logging::directory()?;
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Ok(Vec::new());
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("rambledesk.log")
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.file_name(), entry.path()))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _, _)| *modified);
    let cutoff =
        since.and_then(|_| lookback.and_then(|duration| SystemTime::now().checked_sub(duration)));
    let mut logs = Vec::new();
    for (modified, name, path) in files {
        if cutoff.is_some_and(|bound| modified < bound) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let start = bytes
            .len()
            .saturating_sub(usize::try_from(MAX_LOG_FILE_BYTES).unwrap_or(bytes.len()));
        let raw = String::from_utf8_lossy(&bytes[start..]);
        let sanitized = raw
            .lines()
            .map(redact_log_line)
            .collect::<Vec<_>>()
            .join("\n");
        logs.push((name.to_string_lossy().into_owned(), sanitized));
    }
    Ok(logs)
}

fn write_zip(path: &Path, entries: &[(String, String)]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("无法创建诊断包目录：{error}"))?;
    }
    let file = std::fs::File::create(path)
        .map_err(|error| format!("无法创建诊断包 {}：{error}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for (name, content) in entries {
        zip.start_file(name.as_str(), options)
            .map_err(|error| format!("无法创建诊断包条目 {name}：{error}"))?;
        zip.write_all(content.as_bytes())
            .map_err(|error| format!("无法写入诊断包条目 {name}：{error}"))?;
    }
    zip.finish()
        .map_err(|error| format!("无法完成诊断包 {}：{error}", path.display()))?;
    Ok(())
}

fn pretty_json<T: Serialize>(value: &T, label: &str) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| format!("{label}失败：{error}"))
}

fn with_zip_extension(path: &Path) -> PathBuf {
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("zip"))
    {
        path.to_path_buf()
    } else {
        path.with_extension("zip")
    }
}

fn hours_ago(hours: i64) -> String {
    (OffsetDateTimeNow::now_utc() - time::Duration::hours(hours))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

// Local alias so hours_ago stays readable.
use time::OffsetDateTime as OffsetDateTimeNow;

fn package_readme(report_id: &str, scope: &str) -> String {
    format!(
        "RambleDesk diagnostic package\n\
         Report: {report_id}\n\
         Scope: {scope}\n\n\
         This zip does not contain drafts, feedback markdown, attachments,\n\
         API keys, or local-server tokens. Request titles and source text\n\
         are omitted. Home directories in logs are replaced with %HOME%.\n"
    )
}

fn redact_log_line(line: &str) -> String {
    redact_home(line).chars().take(2_000).collect()
}

pub(crate) fn redact_home(input: &str) -> String {
    let Some(home) = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
    else {
        return input.chars().take(2_000).collect();
    };
    let normalized_input = input.replace('\\', "/");
    let normalized_home = home.to_string_lossy().replace('\\', "/");
    normalized_input.replace(&normalized_home, "%HOME%")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_scope_accepts_frontend_and_legacy_labels() {
        assert_eq!(
            serde_json::from_str::<DiagnosticScope>(r#""last_24_hours""#).expect("24h"),
            DiagnosticScope::LastTwentyFourHours
        );
        assert_eq!(
            serde_json::from_str::<DiagnosticScope>(r#""last_7_days""#).expect("7d"),
            DiagnosticScope::LastSevenDays
        );
        assert_eq!(
            serde_json::from_str::<DiagnosticScope>(r#""last_seven_days""#).expect("legacy"),
            DiagnosticScope::LastSevenDays
        );
        assert_eq!(
            serde_json::from_str::<DiagnosticScope>(r#""all""#).expect("all"),
            DiagnosticScope::All
        );
    }

    #[test]
    fn diagnostic_scope_uses_matching_log_lookbacks() {
        assert_eq!(
            DiagnosticScope::LastTwentyFourHours.lookback(),
            Some(Duration::from_secs(24 * 3600))
        );
        assert_eq!(
            DiagnosticScope::LastSevenDays.lookback(),
            Some(Duration::from_secs(7 * 24 * 3600))
        );
        assert_eq!(DiagnosticScope::All.lookback(), None);
    }

    #[test]
    fn zip_extension_is_normalized() {
        assert_eq!(
            with_zip_extension(Path::new("report")),
            PathBuf::from("report.zip")
        );
        assert_eq!(
            with_zip_extension(Path::new("report.ZIP")),
            PathBuf::from("report.ZIP")
        );
        assert_eq!(
            with_zip_extension(Path::new("report.zip")),
            PathBuf::from("report.zip")
        );
    }

    #[test]
    fn home_paths_are_redacted() {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .expect("home");
        let sample = format!("{}/Library/logs/rambledesk.log", home.display());
        let redacted = redact_home(&sample);
        assert!(redacted.starts_with("%HOME%/"));
        assert!(!redacted.contains(&home.to_string_lossy().replace('\\', "/")));
    }
}
