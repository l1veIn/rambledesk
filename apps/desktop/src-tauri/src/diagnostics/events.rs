//! Local process events for diagnostic export. Metadata only: no drafts,
//! attachments, tokens, titles, or file contents.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const SCHEMA_VERSION: u32 = 1;
const MAX_EVENTS: usize = 20_000;
const TRIM_AT_BYTES: u64 = 4 * 1024 * 1024;

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
    let event = ProcessEvent {
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
    };
    if let Err(error) = record_in(&events_path(), &event) {
        tracing::debug!(%error, "diagnostic event was not persisted");
    }
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
    let raw =
        std::fs::read_to_string(path).map_err(|error| format!("无法读取诊断事件：{error}"))?;
    let lines: Vec<&str> = raw.lines().filter(|line| !line.trim().is_empty()).collect();
    if lines.len() <= MAX_EVENTS {
        return Ok(());
    }
    let mut kept = lines[lines.len() - MAX_EVENTS..].join("\n");
    kept.push('\n');
    let tmp = path.with_extension("jsonl.tmp");
    std::fs::write(&tmp, kept).map_err(|error| format!("无法裁剪诊断事件：{error}"))?;
    std::fs::rename(&tmp, path).map_err(|error| format!("无法落盘诊断事件：{error}"))
}

pub fn list_since(path: &Path, since_rfc3339: Option<&str>) -> Vec<ProcessEvent> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|line| serde_json::from_str::<ProcessEvent>(line).ok())
        .filter(|event| since_rfc3339.is_none_or(|since| event.timestamp.as_str() >= since))
        .collect()
}

pub fn to_csv(events: &[ProcessEvent], app_version: &str) -> String {
    let mut out = String::from(
        "case:concept:name,concept:name,time:timestamp,eventIndex,appSessionId,appVersion,hostId,outcome,errorCode,durationMs\n",
    );
    for event in events {
        let fields = [
            csv(&event.case_id),
            csv(&event.activity),
            csv(&event.timestamp),
            event.event_index.to_string(),
            csv(&event.app_session_id),
            csv(app_version),
            csv(event.host_id.as_deref().unwrap_or("")),
            csv(event.outcome.as_deref().unwrap_or("")),
            csv(event.error_code.as_deref().unwrap_or("")),
            event
                .duration_ms
                .map(|value| value.to_string())
                .unwrap_or_default(),
        ];
        out.push_str(&fields.join(","));
        out.push('\n');
    }
    out
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        let newer = ProcessEvent {
            timestamp: "2026-08-17T00:00:00Z".into(),
            activity: "feedback_submitted".into(),
            event_index: 1,
            ..older.clone()
        };
        record_in(&path, &older).expect("older");
        record_in(&path, &newer).expect("newer");
        assert_eq!(list_since(&path, None).len(), 2);
        assert_eq!(list_since(&path, Some("2026-08-01T00:00:00Z")).len(), 1);
        assert!(to_csv(&list_since(&path, None), "0.0.2").contains("feedback_submitted"));
    }
}
