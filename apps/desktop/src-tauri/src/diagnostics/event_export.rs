use super::events::{MAX_EVENTS, ProcessEvent, read_tail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub source_bytes: u64,
    pub omitted_prefix_bytes: u64,
    pub malformed_lines_skipped: usize,
    pub outside_window_count: usize,
    pub event_limit_dropped: usize,
    pub unavailable: bool,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub app_session_ids: BTreeSet<String>,
}
pub struct EventSnapshot {
    pub events: Vec<ProcessEvent>,
    pub coverage: Coverage,
}

pub fn snapshot_since(path: &Path, since_rfc3339: Option<&str>) -> EventSnapshot {
    let mut coverage = Coverage::default();
    let Ok((bytes, source_bytes, omitted_prefix_bytes)) = read_tail(path) else {
        coverage.unavailable = true;
        return EventSnapshot {
            events: vec![],
            coverage,
        };
    };
    coverage.source_bytes = source_bytes;
    coverage.omitted_prefix_bytes = omitted_prefix_bytes;
    let since = since_rfc3339.and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok());
    let mut events = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let parsed = serde_json::from_slice::<ProcessEvent>(line)
            .ok()
            .and_then(|event| {
                OffsetDateTime::parse(&event.timestamp, &Rfc3339)
                    .ok()
                    .map(|timestamp| (event, timestamp))
            });
        let Some((mut event, timestamp)) = parsed else {
            coverage.malformed_lines_skipped += 1;
            continue;
        };
        if since.is_some_and(|bound| timestamp < bound) {
            coverage.outside_window_count += 1;
            continue;
        }
        // Historical records predate the client whitelist; scrub them again.
        event.case_id = identifier(&event.case_id);
        event.activity = identifier(&event.activity);
        event.app_session_id = identifier(&event.app_session_id);
        event.host_id = event.host_id.map(|value| identifier(&value));
        event.outcome = event.outcome.map(|value| identifier(&value));
        event.error_code = event.error_code.map(|value| identifier(&value));
        event.app_version = event.app_version.map(|value| identifier(&value));
        event.operation_id = event
            .operation_id
            .filter(|value| uuid::Uuid::parse_str(value).is_ok());
        event
            .details
            .retain(|key, value| super::client::valid_detail(key, value));
        events.push((timestamp, event));
    }
    events.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.event_index.cmp(&right.1.event_index))
    });
    coverage.event_limit_dropped = events.len().saturating_sub(MAX_EVENTS);
    let events: Vec<_> = events
        .into_iter()
        .skip(coverage.event_limit_dropped)
        .map(|(_, event)| event)
        .collect();
    coverage.first_timestamp = events.first().map(|event| event.timestamp.clone());
    coverage.last_timestamp = events.last().map(|event| event.timestamp.clone());
    coverage.app_session_ids = events
        .iter()
        .map(|event| event.app_session_id.clone())
        .collect();
    EventSnapshot { events, coverage }
}
fn identifier(value: &str) -> String {
    if value.len() <= 128
        && !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
        && !value.starts_with("sk-")
        && !value.starts_with("rambledesk-session.")
    {
        value.to_owned()
    } else {
        "[OMITTED]".to_owned()
    }
}
pub fn to_jsonl(events: &[ProcessEvent]) -> Result<String, String> {
    let mut out = String::new();
    for event in events {
        out.push_str(&serde_json::to_string(event).map_err(|_| "diagnostic_serialization_failed")?);
        out.push('\n');
    }
    Ok(out)
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySummary {
    event_count: usize,
    failure_count: usize,
    by_outcome: BTreeMap<String, usize>,
    duration_samples: usize,
    total_duration_ms: u64,
    min_duration_ms: Option<u64>,
    max_duration_ms: Option<u64>,
    p50_duration_ms: Option<u64>,
    p95_duration_ms: Option<u64>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    event_count: usize,
    reported_client_events_dropped: u64,
    failure_count: usize,
    operation_count: usize,
    operations_without_terminal_event: usize,
    by_activity: BTreeMap<String, ActivitySummary>,
    by_error_code: BTreeMap<String, usize>,
}
pub fn summarize(events: &[ProcessEvent]) -> Summary {
    let mut summary = Summary {
        event_count: events.len(),
        reported_client_events_dropped: 0,
        failure_count: 0,
        operation_count: 0,
        operations_without_terminal_event: 0,
        by_activity: BTreeMap::new(),
        by_error_code: BTreeMap::new(),
    };
    let mut durations: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    let mut operations = BTreeMap::new();
    for event in events {
        if event.activity == "diagnostic_backpressure" {
            summary.reported_client_events_dropped =
                summary.reported_client_events_dropped.saturating_add(
                    event
                        .details
                        .get("dropped_count")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0),
                );
        }
        let stats = summary
            .by_activity
            .entry(event.activity.clone())
            .or_default();
        stats.event_count += 1;
        *stats
            .by_outcome
            .entry(
                event
                    .outcome
                    .clone()
                    .unwrap_or_else(|| "unspecified".into()),
            )
            .or_default() += 1;
        if matches!(event.outcome.as_deref(), Some("failed" | "error")) {
            stats.failure_count += 1;
            summary.failure_count += 1;
            *summary
                .by_error_code
                .entry(
                    event
                        .error_code
                        .clone()
                        .unwrap_or_else(|| "unspecified".into()),
                )
                .or_default() += 1;
        }
        if let Some(duration) = event.duration_ms {
            durations.entry(&event.activity).or_default().push(duration);
        }
        if let Some(id) = &event.operation_id {
            let ended = operations
                .entry((&event.app_session_id, id))
                .or_insert(false);
            *ended |= matches!(
                event.outcome.as_deref(),
                Some("ok" | "failed" | "error" | "cancelled" | "skipped" | "blocked")
            );
        }
    }
    for (activity, mut samples) in durations {
        samples.sort_unstable();
        let stats = summary.by_activity.get_mut(activity).unwrap();
        stats.duration_samples = samples.len();
        stats.total_duration_ms = samples
            .iter()
            .fold(0_u64, |sum, value| sum.saturating_add(*value));
        stats.min_duration_ms = samples.first().copied();
        stats.max_duration_ms = samples.last().copied();
        stats.p50_duration_ms = Some(samples[(samples.len() * 50).div_ceil(100) - 1]);
        stats.p95_duration_ms = Some(samples[(samples.len() * 95).div_ceil(100) - 1]);
    }
    summary.operation_count = operations.len();
    summary.operations_without_terminal_event =
        operations.values().filter(|ended| !**ended).count();
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(timestamp: &str) -> ProcessEvent {
        let mut event =
            super::super::events::new_event("agent_connection", None, None, Some("ok"), None, None);
        event.timestamp = timestamp.into();
        event
    }
    #[test]
    fn historical_records_are_compatible_and_metadata_is_scrubbed() {
        let old = r#"{"schemaVersion":1,"caseId":"=private","activity":"ramble_started","timestamp":"2026-01-01T00:00:00Z","appSessionId":"s","details":{"prompt":"private task"}}"#;
        let parsed: ProcessEvent = serde_json::from_str(old).unwrap();
        assert!(parsed.app_version.is_none());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, old).unwrap();
        let snapshot = snapshot_since(&path, None);
        assert_eq!(snapshot.events[0].case_id, "[OMITTED]");
        assert!(snapshot.events[0].details.is_empty());
        assert!(!to_jsonl(&snapshot.events).unwrap().contains("private"));
    }
    #[test]
    fn timestamps_offsets_malformed_lines_and_oversized_prefix_are_accounted_for() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut bytes = vec![b'x'; super::super::events::TRIM_AT_BYTES as usize + 10];
        bytes.push(b'\n');
        bytes.extend_from_slice(
            to_jsonl(&[
                event("2026-01-01T08:00:00+08:00"),
                event("2026-01-01T01:00:00Z"),
            ])
            .unwrap()
            .as_bytes(),
        );
        bytes.extend_from_slice(b"broken\n");
        std::fs::write(&path, bytes).unwrap();
        let snapshot = snapshot_since(&path, Some("2026-01-01T00:30:00Z"));
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(snapshot.coverage.outside_window_count, 1);
        assert_eq!(snapshot.coverage.malformed_lines_skipped, 1);
        assert!(snapshot.coverage.omitted_prefix_bytes > 0);
    }
    #[test]
    fn summary_groups_failures_durations_and_incomplete_operations() {
        let mut start = event("2026-01-01T00:00:00Z");
        start.outcome = Some("started".into());
        start.operation_id = Some(uuid::Uuid::now_v7().to_string());
        let mut end = start.clone();
        end.outcome = Some("failed".into());
        end.error_code = Some("timeout".into());
        end.duration_ms = Some(100);
        let mut unfinished = start.clone();
        unfinished.operation_id = Some(uuid::Uuid::now_v7().to_string());
        let summary = summarize(&[start, end, unfinished]);
        assert_eq!(summary.failure_count, 1);
        assert_eq!(summary.operation_count, 2);
        assert_eq!(summary.operations_without_terminal_event, 1);
        assert_eq!(
            summary.by_activity["agent_connection"].p95_duration_ms,
            Some(100)
        );
    }
}
