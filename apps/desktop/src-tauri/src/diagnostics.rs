//! Privacy-preserving diagnostic package export.
//!
//! The zip contains environment, runtime, model, request metadata, usage
//! events, and redacted logs. It never copies drafts, feedback markdown,
//! attachments, tokens, API keys, or request titles.

mod client;
pub(crate) mod control;
mod event_export;
mod events;
mod logs;
mod package;

use std::path::Path;
use std::time::Duration;

use rambledesk_core::{FeedbackApplication, FeedbackStatus, ListFeedbackRequestsInput};
use rambledesk_local_server::{WebAccessSecurityLimits, WebAccessServerConfig};
use rambledesk_speech::model::list_models;
use serde::Serialize;
use tauri::AppHandle;

use crate::WorkbenchState;
use crate::config::load_storage_preferences;
use crate::macos_permissions::list_macos_permissions;

pub use client::ClientDiagnosticInput;
pub(crate) use events::record as record_event;
pub(crate) use logs::redact_home;
use logs::{LogCoverage, MAX_LOG_FILE_BYTES, MAX_LOG_FILES, collect_logs};
use package::{
    hours_ago, package_readme, pretty_json, usage_summary, with_zip_extension, write_zip,
};

const PACKAGE_SCHEMA_VERSION: u32 = 2;
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
    app_session_id: &'a str,
    requested_since: Option<&'a str>,
    event_count: usize,
    event_coverage: &'a event_export::Coverage,
    log_coverage: &'a LogCoverage,
    max_event_file_bytes: u64,
    max_exported_events: usize,
    max_log_files: usize,
    max_log_file_bytes: u64,
    request_limit_reached: bool,
    debug_build: bool,
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
    web_access: WebAccessRuntimeSnapshot,
    speech_session_active: bool,
    macos_permissions: Vec<PermissionSnapshot>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebAccessRuntimeSnapshot {
    state: String,
    loopback_only: bool,
    fixed_port: u16,
    failure_code: Option<String>,
    security_limits: WebAccessSecurityLimitsSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebAccessSecurityLimitsSnapshot {
    max_bootstrap_attempts_per_minute: usize,
    max_http_requests: usize,
    max_event_connections: usize,
    max_json_body_bytes: usize,
    max_attachment_upload_body_bytes: usize,
    session_idle_timeout_seconds: u64,
    session_absolute_timeout_seconds: u64,
    max_sessions: usize,
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
    // Diagnostics must not discover external integrations. Only an explicit
    // visit to External adapters may scan their installations and configuration.
    // null means uncollected, rather than an empty list of detected adapters.
    let adapters = serde_json::Value::Null;
    let requests = request_metadata(&state.application, since.as_deref()).await?;
    let event_snapshot = events::snapshot(since.as_deref());
    let events = &event_snapshot.events;
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
        app_session_id: events::app_session_id(),
        requested_since: since.as_deref(),
        event_count: events.len(),
        event_coverage: &event_snapshot.coverage,
        log_coverage: &logs.coverage,
        max_event_file_bytes: events::TRIM_AT_BYTES,
        max_exported_events: events::MAX_EVENTS,
        max_log_files: MAX_LOG_FILES,
        max_log_file_bytes: MAX_LOG_FILE_BYTES,
        request_limit_reached: requests.len() >= MAX_REQUESTS,
        debug_build: cfg!(debug_assertions),
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
            events::to_csv(events, &app_version),
        ),
        ("events.jsonl".to_owned(), event_export::to_jsonl(events)?),
        (
            "event-summary.json".to_owned(),
            pretty_json(&event_export::summarize(events), "序列化事件摘要")?,
        ),
        (
            "README.txt".to_owned(),
            package_readme(&report_id, scope.as_label()),
        ),
    ];
    for (name, contents) in logs.entries {
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
pub fn record_client_diagnostic(input: ClientDiagnosticInput) -> Result<(), String> {
    client::record(input)
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
    let web_access_state = state
        .web_access_lifecycle
        .lock()
        .await
        .diagnostic_state()
        .await;
    let web_access = web_access_runtime_snapshot(
        web_access_state,
        WebAccessServerConfig::default().security_limits(),
    );
    let library_root = state.library_root();
    RuntimeSnapshot {
        app_session_id: events::app_session_id().to_owned(),
        data_storage_customized: customized,
        library_root: redact_home(&library_root.display().to_string()),
        local_server_loopback: true,
        web_access,
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

fn web_access_runtime_snapshot(
    state: crate::web_access::WebAccessDiagnosticState,
    limits: WebAccessSecurityLimits,
) -> WebAccessRuntimeSnapshot {
    WebAccessRuntimeSnapshot {
        state: state.state.to_owned(),
        loopback_only: limits.loopback_address.is_loopback(),
        fixed_port: limits.port,
        failure_code: state.failure_code.map(ToOwned::to_owned),
        security_limits: WebAccessSecurityLimitsSnapshot {
            max_bootstrap_attempts_per_minute: limits.max_bootstrap_attempts_per_minute,
            max_http_requests: limits.max_http_requests,
            max_event_connections: limits.max_event_connections,
            max_json_body_bytes: limits.max_json_body_bytes,
            max_attachment_upload_body_bytes: limits.max_attachment_upload_body_bytes,
            session_idle_timeout_seconds: limits.session_idle_timeout_seconds,
            session_absolute_timeout_seconds: limits.session_absolute_timeout_seconds,
            max_sessions: limits.max_sessions,
        },
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::path::PathBuf;

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
    fn zip_contains_matching_jsonl_csv_and_summary_for_offline_analysis() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.zip");
        let mut event = events::new_event(
            "onboarding_action",
            None,
            None,
            Some("failed"),
            Some("timeout"),
            Some(100),
        );
        event.operation_id = Some(uuid::Uuid::now_v7().to_string());
        event.details.insert("step".into(), "agents".into());
        let events = vec![event];
        write_zip(
            &path,
            &[
                (
                    "events.jsonl".into(),
                    event_export::to_jsonl(&events).unwrap(),
                ),
                (
                    "events.csv".into(),
                    events::to_csv(&events, "export-version"),
                ),
                (
                    "event-summary.json".into(),
                    pretty_json(&event_export::summarize(&events), "summary").unwrap(),
                ),
            ],
        )
        .unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut jsonl = String::new();
        zip.by_name("events.jsonl")
            .unwrap()
            .read_to_string(&mut jsonl)
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(jsonl.trim()).unwrap();
        assert_eq!(parsed["details"]["step"], "agents");
        assert_eq!(parsed["schemaVersion"], 2);
        let mut summary = String::new();
        zip.by_name("event-summary.json")
            .unwrap()
            .read_to_string(&mut summary)
            .unwrap();
        let summary: serde_json::Value = serde_json::from_str(&summary).unwrap();
        assert_eq!(summary["eventCount"], 1);
        assert_eq!(summary["failureCount"], 1);
        let mut csv = String::new();
        zip.by_name("events.csv")
            .unwrap()
            .read_to_string(&mut csv)
            .unwrap();
        assert_eq!(csv.lines().count(), 2);
        assert!(csv.contains(parsed["operationId"].as_str().unwrap()));
    }

    #[test]
    fn web_access_runtime_diagnostics_expose_limits_without_address_or_credentials() {
        let limits = WebAccessServerConfig::default().security_limits();
        let snapshot = web_access_runtime_snapshot(
            crate::web_access::WebAccessDiagnosticState {
                state: "failed",
                failure_code: Some("listener_failed"),
            },
            limits,
        );
        let value = serde_json::to_value(snapshot).expect("serialize Web Access diagnostics");

        assert_eq!(value["state"], "failed");
        assert_eq!(value["loopbackOnly"], true);
        assert_eq!(value["fixedPort"], limits.port);
        assert_eq!(value["failureCode"], "listener_failed");
        assert_eq!(
            value["securityLimits"]["maxBootstrapAttemptsPerMinute"],
            limits.max_bootstrap_attempts_per_minute
        );
        assert_eq!(
            value["securityLimits"]["maxHttpRequests"],
            limits.max_http_requests
        );
        assert_eq!(
            value["securityLimits"]["maxEventConnections"],
            limits.max_event_connections
        );
        assert_eq!(
            value["securityLimits"]["maxJsonBodyBytes"],
            limits.max_json_body_bytes
        );
        assert_eq!(
            value["securityLimits"]["maxAttachmentUploadBodyBytes"],
            limits.max_attachment_upload_body_bytes
        );
        assert_eq!(
            value["securityLimits"]["sessionIdleTimeoutSeconds"],
            limits.session_idle_timeout_seconds
        );
        assert_eq!(
            value["securityLimits"]["sessionAbsoluteTimeoutSeconds"],
            limits.session_absolute_timeout_seconds
        );
        assert_eq!(value["securityLimits"]["maxSessions"], limits.max_sessions);
        let serialized = value.to_string();
        for forbidden in [
            "127.0.0.1",
            "http://",
            "authorization",
            "bearer",
            "rambledesk-session.",
            "session_token",
            "durable_token",
        ] {
            assert!(!serialized.to_ascii_lowercase().contains(forbidden));
        }
    }
}
