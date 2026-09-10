//! Zip packaging and human-readable summary for diagnostic exports.
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use time::OffsetDateTime as OffsetDateTimeNow;
use zip::write::SimpleFileOptions;

use super::{RequestMetadata, UsageSummary};

pub(super) fn usage_summary(requests: &[RequestMetadata]) -> UsageSummary {
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

pub(super) fn increment(map: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    let count = map
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    map.insert(key.to_owned(), serde_json::json!(count + 1));
}

pub(super) fn write_zip(path: &Path, entries: &[(String, String)]) -> Result<(), String> {
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

pub(super) fn pretty_json<T: Serialize>(value: &T, label: &str) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| format!("{label}失败：{error}"))
}

pub(super) fn with_zip_extension(path: &Path) -> PathBuf {
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

pub(super) fn hours_ago(hours: i64) -> String {
    (OffsetDateTimeNow::now_utc() - time::Duration::hours(hours))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

pub(super) fn package_readme(report_id: &str, scope: &str) -> String {
    format!(
        "RambleDesk diagnostic package\n\
         Report: {report_id}\n\
         Scope: {scope}\n\n\
         Start with manifest.json (build, app session, requested range and coverage),\n\
         then event-summary.json (activity/outcome/failure counts and duration percentiles).\n\
         events.jsonl preserves typed metadata; events.csv is the same event set for tables.\n\
         Correlate UI operations by appSessionId + operationId; order within a session\n\
         by eventIndex and compare UTC timestamps. Details such as step/action/phase\n\
         identify onboarding, model downloads, agent checks and session preparation.\n\
         Search logs for 'agent operation': native ACP INFO records have operation_id,\n\
         stable IDs, phase/status/error_code/elapsed_ms. Native and UI operation IDs\n\
         are separate scopes; correlate these using stable IDs and nearby timestamps.\n\n\
         Durations summarize events carrying durationMs, not inferred wall-clock spans.\n\
         Failed/error outcomes count as failures; skipped/blocked/cancelled stay separate.\n\
         An operation without a terminal event can be running, interrupted, dropped, or\n\
         outside the retained window. It is not proof of an authentication failure.\n\n\
         Retention: the event file trims from 4 MiB to at most 3 MiB / 20,000 records.\n\
         Export reads at most the latest 4 MiB and 20,000 events. Earlier trims are\n\
         irreversible and their total historical counts are unknown. See eventCoverage\n\
         for actual first/last times, invalid records, and export truncation.\n\
         Logs retain up to 7 files at startup; export includes at most 7 newest eligible\n\
         files, each limited to its last 1 MiB of complete lines and 2,000 chars/line.\n\
         Each redacted output is also capped at 1 MiB; omitted lines are counted.\n\
         Log scope filters file modification time, so a retained file may include older\n\
         lines. Incomplete first/last lines are omitted; see logCoverage for counts.\n\
         Requests are limited to 2,000 metadata records. All means all retained evidence.\n\
         Old events lacking appVersion are unknown, not assigned the exporting version.\n\n\
         Drafts, feedback bodies, attachments and request titles are not copied.\n\
         Client events accept only a shared static vocabulary and numeric/boolean metadata.\n\
         Logs redact credential patterns and home directories (%HOME%); old frontend\n\
         error lines and unstructured continuations omit their free text.\n\
         Raw error/message/body/prompt fields are omitted.\n\
         Review the package before sharing; application logs may still contain operational\n\
         paths or third-party text outside recognized sensitive fields.\n\
         Export reads existing evidence only. No Agent or external adapter is scanned\n\
         or installed. External adapter state is uncollected; adapters.json is null.\n"
    )
}
