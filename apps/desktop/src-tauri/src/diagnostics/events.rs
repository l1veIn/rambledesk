//! Local process events for diagnostic export. Metadata only: no drafts,
//! attachments, tokens, titles, or file contents.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const SCHEMA_VERSION: u32 = 2;
pub const MAX_EVENTS: usize = 20_000;
pub const TRIM_AT_BYTES: u64 = 4 * 1024 * 1024;
const RETAIN_BYTES: usize = 3 * 1024 * 1024;
const MAX_EVENT_BYTES: usize = 8 * 1024;

static WRITE_LOCK: Mutex<()> = Mutex::new(());
static APP_SESSION_ID: OnceLock<String> = OnceLock::new();
static EVENT_INDEX: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEvent {
    pub schema_version: u32,
    pub case_id: String,
    pub activity: String,
    pub timestamp: String,
    pub app_session_id: String,
    #[serde(default)]
    pub event_index: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, serde_json::Value>,
}

pub fn app_session_id() -> &'static str {
    APP_SESSION_ID
        .get_or_init(|| format!("app-{}", uuid::Uuid::now_v7()))
        .as_str()
}

pub fn events_path() -> PathBuf {
    rambledesk_storage::default_app_data_root()
        .map(|root| root.join("diagnostics").join("events.jsonl"))
        .unwrap_or_else(|_| PathBuf::from("diagnostics/events.jsonl"))
}

pub fn utc_now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

pub fn record(
    activity: &str,
    case_id: Option<&str>,
    host_id: Option<&str>,
    outcome: Option<&str>,
    error_code: Option<&str>,
    duration_ms: Option<u64>,
) {
    let event = new_event(activity, case_id, host_id, outcome, error_code, duration_ms);
    persist(&event);
}

pub fn new_event(
    activity: &str,
    case_id: Option<&str>,
    host_id: Option<&str>,
    outcome: Option<&str>,
    error_code: Option<&str>,
    duration_ms: Option<u64>,
) -> ProcessEvent {
    ProcessEvent {
        schema_version: SCHEMA_VERSION,
        case_id: case_id
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| app_session_id())
            .to_owned(),
        activity: activity.to_owned(),
        timestamp: utc_now_rfc3339(),
        app_session_id: app_session_id().to_owned(),
        event_index: EVENT_INDEX.fetch_add(1, Ordering::Relaxed),
        host_id: host_id
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        outcome: outcome.map(ToOwned::to_owned),
        error_code: error_code.map(ToOwned::to_owned),
        duration_ms,
        app_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        operation_id: None,
        details: BTreeMap::new(),
    }
}

pub fn persist(event: &ProcessEvent) {
    if persist_recorded_event(&events_path(), event).is_err() {
        tracing::debug!("diagnostic event was not persisted");
    }
}

pub(super) fn persist_recorded_event(path: &Path, event: &ProcessEvent) -> Result<(), String> {
    let Ok(control) = super::control::global() else {
        return Ok(());
    };
    record_if_enabled(control, path, event)
}

pub(super) fn record_if_enabled(
    control: &super::control::DiagnosticsControl,
    path: &Path,
    event: &ProcessEvent,
) -> Result<(), String> {
    let Some(_recording) = control.recording_guard() else {
        return Ok(());
    };
    record_in(path, event)
}

pub fn record_in(path: &Path, event: &ProcessEvent) -> Result<(), String> {
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建诊断事件目录：{error}"))?;
    }
    let line =
        serde_json::to_string(event).map_err(|error| format!("无法序列化诊断事件：{error}"))?;
    if line.len() + 1 > MAX_EVENT_BYTES {
        return Err("diagnostic_event_too_large".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("无法打开诊断事件文件：{error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("无法写入诊断事件：{error}"))?;
    drop(file);
    if std::fs::metadata(path)
        .map(|metadata| metadata.len() > TRIM_AT_BYTES)
        .unwrap_or(false)
    {
        trim_in(path)?;
    }
    Ok(())
}

fn trim_in(path: &Path) -> Result<(), String> {
    let (bytes, _, _) = read_tail(path).map_err(|_| "diagnostic_read_failed")?;
    let raw = String::from_utf8_lossy(&bytes);
    let mut size = 0;
    let mut lines = Vec::new();
    for line in raw
        .lines()
        .rev()
        .filter(|line| !line.is_empty())
        .take(MAX_EVENTS)
    {
        if size + line.len() + 1 > RETAIN_BYTES {
            break;
        }
        size += line.len() + 1;
        lines.push(line);
    }
    lines.reverse();
    let mut kept = lines.join("\n");
    kept.push('\n');
    let tmp = path.with_extension("jsonl.tmp");
    std::fs::write(&tmp, kept).map_err(|error| format!("无法裁剪诊断事件：{error}"))?;
    std::fs::rename(&tmp, path).map_err(|error| format!("无法落盘诊断事件：{error}"))
}

/// Bounded reads also handle oversized files created by older builds.
pub fn read_tail(path: &Path) -> std::io::Result<(Vec<u8>, u64, u64)> {
    let mut file = std::fs::File::open(path)?;
    let source_bytes = file.metadata()?.len();
    let start = source_bytes.saturating_sub(TRIM_AT_BYTES);
    file.seek(SeekFrom::Start(start.saturating_sub(1)))?;
    let mut bytes = Vec::new();
    file.take(TRIM_AT_BYTES + 1).read_to_end(&mut bytes)?;
    let skipped = if start == 0 {
        0
    } else if bytes.first() == Some(&b'\n') {
        1
    } else {
        bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |index| index + 1)
    };
    Ok((
        bytes[skipped..].to_vec(),
        source_bytes,
        start.saturating_sub(1) + skipped as u64,
    ))
}

pub fn snapshot(since_rfc3339: Option<&str>) -> super::event_export::EventSnapshot {
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    super::event_export::snapshot_since(&events_path(), since_rfc3339)
}

pub fn to_csv(events: &[ProcessEvent], app_version: &str) -> String {
    let mut out = String::from(
        "case:concept:name,concept:name,time:timestamp,eventIndex,appSessionId,appVersion,hostId,outcome,errorCode,durationMs,operationId,details,schemaVersion,exportAppVersion\n",
    );
    for event in events {
        let fields = [
            csv(&event.case_id),
            csv(&event.activity),
            csv(&event.timestamp),
            event.event_index.to_string(),
            csv(&event.app_session_id),
            csv(event.app_version.as_deref().unwrap_or("")),
            csv(event.host_id.as_deref().unwrap_or("")),
            csv(event.outcome.as_deref().unwrap_or("")),
            csv(event.error_code.as_deref().unwrap_or("")),
            event
                .duration_ms
                .map(|value| value.to_string())
                .unwrap_or_default(),
            csv(event.operation_id.as_deref().unwrap_or("")),
            csv(&serde_json::to_string(&event.details).unwrap_or_default()),
            event.schema_version.to_string(),
            csv(app_version),
        ];
        out.push_str(&fields.join(","));
        out.push('\n');
    }
    out
}

fn csv(value: &str) -> String {
    let safe = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    let value = safe.as_str();
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_caps_bytes_below_twenty_thousand_records_and_rejects_huge_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut event = new_event("app_started", None, None, Some("ok"), None, None);
        let line = serde_json::to_string(&event).unwrap() + "\n";
        let count = TRIM_AT_BYTES as usize / line.len() + 2;
        assert!(count < MAX_EVENTS);
        std::fs::write(&path, line.repeat(count)).unwrap();
        record_in(&path, &event).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() <= RETAIN_BYTES as u64);
        let snapshot = super::super::event_export::snapshot_since(&path, None);
        assert_eq!(snapshot.coverage.malformed_lines_skipped, 0);
        event.case_id = "x".repeat(MAX_EVENT_BYTES);
        assert!(record_in(&path, &event).is_err());
        assert_eq!(csv("=formula"), "'=formula");
    }

    #[test]
    fn records_and_filters_events_by_timestamp() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("events.jsonl");
        let older = ProcessEvent {
            schema_version: 1,
            case_id: "a".into(),
            activity: "app_started".into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            app_session_id: "s".into(),
            event_index: 0,
            host_id: None,
            outcome: None,
            error_code: None,
            duration_ms: None,
            app_version: None,
            operation_id: None,
            details: BTreeMap::new(),
        };
        let newer = ProcessEvent {
            timestamp: "2026-08-17T00:00:00Z".into(),
            activity: "feedback_submitted".into(),
            event_index: 1,
            ..older.clone()
        };
        record_in(&path, &older).expect("older");
        record_in(&path, &newer).expect("newer");
        let snapshot = super::super::event_export::snapshot_since;
        assert_eq!(snapshot(&path, None).events.len(), 2);
        assert_eq!(
            snapshot(&path, Some("2026-08-01T00:00:00Z")).events.len(),
            1
        );
        assert!(to_csv(&snapshot(&path, None).events, "0.0.2").contains("feedback_submitted"));
    }

    #[test]
    fn disabled_recording_drops_native_and_frontend_events_without_creating_files() {
        let directory = tempfile::tempdir().unwrap();
        let control = super::super::control::DiagnosticsControl::load(directory.path().to_owned());
        let path = directory.path().join("diagnostics/events.jsonl");
        let native = new_event("app_started", None, None, Some("ok"), None, None);
        let mut client = new_event("onboarding_action", None, None, Some("ok"), None, None);
        client.operation_id = Some(uuid::Uuid::now_v7().to_string());
        control.set_enabled(false).unwrap();
        record_if_enabled(&control, &path, &native).unwrap();
        record_if_enabled(&control, &path, &client).unwrap();
        assert!(!path.exists());
        control.set_enabled(true).unwrap();
        record_if_enabled(&control, &path, &native).unwrap();
        record_if_enabled(&control, &path, &client).unwrap();
        assert_eq!(
            super::super::event_export::snapshot_since(&path, None)
                .events
                .len(),
            2
        );
        control.clear().unwrap();
        assert_eq!(
            super::super::event_export::snapshot_since(&path, None)
                .events
                .len(),
            0
        );
        record_if_enabled(&control, &path, &native).unwrap();
        assert_eq!(
            super::super::event_export::snapshot_since(&path, None)
                .events
                .len(),
            1
        );
    }
}
