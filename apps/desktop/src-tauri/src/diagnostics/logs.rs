//! Bounded, redacted log evidence for offline diagnostic packages.
use crate::logging;
use serde::Serialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub(super) const MAX_LOG_FILE_BYTES: u64 = 1024 * 1024;
pub(super) const MAX_LOG_FILES: usize = 7;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LogCoverage {
    unavailable: bool,
    files_omitted_by_limit: usize,
    files_outside_window: usize,
    files_unreadable: usize,
    files: Vec<LogFileCoverage>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogFileCoverage {
    name: String,
    source_bytes: u64,
    omitted_prefix_bytes: u64,
    omitted_partial_suffix_bytes: usize,
    exported_lines: usize,
    truncated_lines: usize,
    omitted_legacy_frontend_lines: usize,
    omitted_unstructured_lines: usize,
    output_limit_dropped_lines: usize,
}
pub(super) struct LogSnapshot {
    pub(super) entries: Vec<(String, String)>,
    pub(super) coverage: LogCoverage,
}

pub(super) fn collect_logs(
    since: Option<&str>,
    lookback: Option<Duration>,
) -> Result<LogSnapshot, String> {
    let directory = logging::directory()?;
    collect_logs_in(&directory, since, lookback)
}

fn collect_logs_in(
    directory: &Path,
    since: Option<&str>,
    lookback: Option<Duration>,
) -> Result<LogSnapshot, String> {
    let mut coverage = LogCoverage::default();
    let Ok(entries) = std::fs::read_dir(directory) else {
        coverage.unavailable = true;
        return Ok(LogSnapshot {
            entries: vec![],
            coverage,
        });
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
            if !entry.file_type().ok()?.is_file() {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.file_name(), entry.path()))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _, _)| *modified);
    let cutoff =
        since.and_then(|_| lookback.and_then(|duration| SystemTime::now().checked_sub(duration)));
    let mut logs = Vec::new();
    for (modified, name, path) in files.into_iter().rev() {
        if cutoff.is_some_and(|bound| modified < bound) {
            coverage.files_outside_window += 1;
            continue;
        }
        if logs.len() >= MAX_LOG_FILES {
            coverage.files_omitted_by_limit += 1;
            continue;
        }
        let Ok((bytes, source_bytes, omitted_prefix_bytes)) = read_log_tail(&path) else {
            coverage.files_unreadable += 1;
            continue;
        };
        // A currently written final line may contain only part of a credential marker.
        let complete_end = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let raw = String::from_utf8_lossy(&bytes[..complete_end]);
        let lines: Vec<_> = raw.lines().collect();
        let (sanitized, exported_lines, output_limit_dropped_lines) = sanitize_log_lines(&raw);
        let name = name.to_string_lossy().into_owned();
        coverage.files.push(LogFileCoverage {
            name: name.clone(),
            source_bytes,
            omitted_prefix_bytes,
            omitted_partial_suffix_bytes: bytes.len() - complete_end,
            exported_lines,
            truncated_lines: lines
                .iter()
                .filter(|line| line.chars().count() > 2_000)
                .count(),
            omitted_legacy_frontend_lines: lines
                .iter()
                .filter(|line| line.contains("frontend error"))
                .count(),
            omitted_unstructured_lines: lines
                .iter()
                .filter(|line| !is_timestamped_log(line))
                .count(),
            output_limit_dropped_lines,
        });
        logs.push((name, sanitized));
    }
    Ok(LogSnapshot {
        entries: logs,
        coverage,
    })
}

fn sanitize_log_lines(raw: &str) -> (String, usize, usize) {
    let mut out = String::new();
    let mut exported = 0;
    let mut omitted = 0;
    for line in raw.lines().filter(|line| is_timestamped_log(line)) {
        let sanitized = redact_log_line(line);
        // Redaction can expand tiny values. Bound the transformed output too,
        // and count omitted lines instead of expanding each into a placeholder.
        if out.len() + sanitized.len() + 1 > MAX_LOG_FILE_BYTES as usize {
            omitted += 1;
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&sanitized);
        exported += 1;
    }
    (out, exported, omitted)
}

fn read_log_tail(path: &Path) -> std::io::Result<(Vec<u8>, u64, u64)> {
    let mut file = std::fs::File::open(path)?;
    let source_bytes = file.metadata()?.len();
    let start = source_bytes.saturating_sub(MAX_LOG_FILE_BYTES);
    let read_start = start.saturating_sub(1);
    file.seek(SeekFrom::Start(read_start))?;
    let mut bytes = Vec::new();
    file.take(MAX_LOG_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    let complete = complete_log_tail(&bytes, usize::from(start > 0));
    let omitted = read_start + (bytes.len() - complete.len()) as u64;
    Ok((complete.to_vec(), source_bytes, omitted))
}

fn complete_log_tail(bytes: &[u8], start: usize) -> &[u8] {
    let tail = &bytes[start..];
    if start == 0 || bytes.get(start - 1) == Some(&b'\n') {
        return tail;
    }
    tail.iter()
        .position(|byte| *byte == b'\n')
        .map_or(&[], |newline| &tail[newline + 1..])
}

fn is_timestamped_log(line: &str) -> bool {
    line.split_whitespace().next().is_some_and(|timestamp| {
        time::OffsetDateTime::parse(timestamp, &time::format_description::well_known::Rfc3339)
            .is_ok()
    })
}

fn redact_log_line(line: &str) -> String {
    if line.contains("frontend error") {
        // Historical versions wrote arbitrary JS error messages, possibly multiline.
        return "[OMITTED: legacy frontend error text]".into();
    }
    // Parse quoted sensitive fields before path normalization can change their
    // escape sequences and expose the remainder of a quoted value.
    redact_home(&redact_log_credentials(line))
        .chars()
        .take(2_000)
        .collect()
}

fn redact_log_credentials(input: &str) -> String {
    let without_bearer = redact_prefixed_credential(input, "bearer ");
    let mut output = redact_prefixed_credential(&without_bearer, "rambledesk-session.");
    for prefix in ["sk-", "sk_ant_", "ghp_", "github_pat_"] {
        output = redact_prefixed_credential(&output, prefix);
    }
    for key in [
        "api_key",
        "apikey",
        "api-key",
        "access_token",
        "refresh_token",
        "session_token",
        "token",
        "password",
        "secret",
        "authorization",
        "prompt",
        "body",
        "content",
        "message",
        "error",
    ] {
        output = redact_assignment(&output, key);
    }
    output
}

fn redact_assignment(input: &str, key: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    let mut search = 0;
    while let Some(found) = find_ascii_case_insensitive(&bytes[search..], key.as_bytes()) {
        let start = search + found;
        search = start + key.len();
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        let mut value = search;
        if matches!(bytes.get(value), Some(b'"' | b'\'')) {
            value += 1;
        }
        while bytes.get(value).is_some_and(u8::is_ascii_whitespace) {
            value += 1;
        }
        if !matches!(bytes.get(value), Some(b'=' | b':')) {
            continue;
        }
        value += 1;
        while bytes.get(value).is_some_and(u8::is_ascii_whitespace) {
            value += 1;
        }
        // Preserve authorization labels whose values were already removed above.
        let unquoted = value + usize::from(matches!(bytes.get(value), Some(b'"' | b'\'')));
        if key == "authorization"
            && find_ascii_case_insensitive(&bytes[unquoted..], b"bearer ") == Some(0)
        {
            continue;
        }
        let end = if matches!(bytes.get(value), Some(b'"' | b'\'')) {
            let quote = bytes[value];
            value += 1;
            let mut end = value;
            while end < bytes.len() {
                if bytes[end] == b'\\' {
                    end = (end + 2).min(bytes.len());
                } else if bytes[end] == quote {
                    break;
                } else {
                    end += 1;
                }
            }
            end
        } else {
            // Unquoted tracing Display values can contain spaces; the remainder
            // has no reliable content boundary, so do not preserve a partial error.
            bytes.len()
        };
        out.push_str(&input[cursor..value]);
        out.push_str("[REDACTED]");
        cursor = end;
        search = end;
    }
    out.push_str(&input[cursor..]);
    out
}

fn redact_prefixed_credential(input: &str, prefix: &str) -> String {
    let bytes = input.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(relative_start) = find_ascii_case_insensitive(&bytes[cursor..], prefix_bytes) {
        let prefix_start = cursor + relative_start;
        let credential_start = prefix_start + prefix_bytes.len();
        if prefix_start > 0 && bytes[prefix_start - 1].is_ascii_alphanumeric() {
            output.push_str(&input[cursor..credential_start]);
            cursor = credential_start;
            continue;
        }
        output.push_str(&input[cursor..credential_start]);
        let credential_end = bytes[credential_start..]
            .iter()
            .position(|byte| credential_delimiter(*byte))
            .map_or(bytes.len(), |offset| credential_start + offset);
        if credential_end == credential_start {
            cursor = credential_start;
            continue;
        }
        output.push_str("[REDACTED]");
        cursor = credential_end;
    }
    output.push_str(&input[cursor..]);
    output
}

fn find_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    })
}

fn credential_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(byte, b'"' | b'\'' | b',' | b';' | b')' | b']' | b'}' | b'>')
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

    #[test]
    fn exported_log_lines_redact_bearer_and_websocket_protocol_credentials() {
        let durable_token = "a".repeat(64);
        let session_token = "session-token_value";
        let line = format!(
            "Authorization: Bearer {durable_token}; Sec-WebSocket-Protocol: rambledesk-events, rambledesk-session.{session_token}"
        );

        let redacted = redact_log_line(&line);

        assert_eq!(
            redacted,
            "Authorization: Bearer [REDACTED]; Sec-WebSocket-Protocol: rambledesk-events, rambledesk-session.[REDACTED]"
        );
        assert!(!redacted.contains(&durable_token));
        assert!(!redacted.contains(session_token));
    }

    #[test]
    fn credential_redaction_handles_json_and_header_case_without_touching_labels() {
        let line = r#"{"authorization":"bEaReR secret-token","protocol":"RAMBLEDESK-SESSION.another_secret"}"#;
        assert_eq!(
            redact_log_line(line),
            r#"{"authorization":"bEaReR [REDACTED]","protocol":"RAMBLEDESK-SESSION.[REDACTED]"}"#
        );
    }

    #[test]
    fn truncated_log_tails_drop_the_partial_first_line_before_redaction() {
        let bytes = b"Authorization: Bearer secret-token\nsafe next line\n";
        assert_eq!(
            complete_log_tail(bytes, "Authorization: Bearer ".len()),
            b"safe next line\n"
        );
        assert_eq!(complete_log_tail(bytes, 0), bytes);
        assert_eq!(
            complete_log_tail(bytes, "Authorization: Bearer secret-token\n".len()),
            b"safe next line\n"
        );
    }

    #[test]
    fn legacy_free_text_and_credential_assignments_are_removed() {
        for line in [
            r#"api_key="private token" password='another secret'"#,
            r#"{"apiKey":"private token","prompt":"private task with \"quotes\""}"#,
            "error=failed while sending private task and credential",
            "message=private user text with spaces",
            "frontend error context=window message=private task",
            "Authorization: Basic private-credential",
            "credential sk-privatekey",
        ] {
            let redacted = redact_log_line(line);
            assert!(!redacted.contains("private"), "{redacted}");
            assert!(!redacted.contains("another secret"));
        }
        let native = "2026-01-01T00:00:00Z INFO agent operation operation_id=00000000-0000-4000-8000-000000000000 phase=connect status=failed error_code=timeout elapsed_ms=120";
        assert_eq!(redact_log_line(native), native);
    }

    #[test]
    fn sensitive_quoted_values_are_redacted_before_normalizing_backslashes() {
        for line in [
            r#"error="failed \"private task\" details" phase=connect"#,
            r#"api_key="prefix \"private credential\" suffix" status=failed"#,
            r#"{"message":"failed \\server\\folder \"private task\" details","phase":"connect"}"#,
            r#"{"apiKey":"prefix \\ \"private credential\" suffix","status":"failed"}"#,
        ] {
            let redacted = redact_log_line(line);
            assert!(!redacted.contains("private"), "{redacted}");
            assert!(!redacted.contains("prefix"), "{redacted}");
            assert!(!redacted.contains("suffix"), "{redacted}");
            assert!(!redacted.contains("details"), "{redacted}");
            assert!(redacted.contains("[REDACTED]"), "{redacted}");
        }
    }

    #[test]
    fn bounded_log_export_accounts_for_partial_lines_and_legacy_multiline_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rambledesk.log.2026-01-01");
        let mut bytes = vec![b'x'; MAX_LOG_FILE_BYTES as usize + 100];
        bytes.extend_from_slice(b"\n2026-01-01T00:00:00Z ERROR frontend error message=private\nprivate continuation\n2026-01-01T00:00:01Z INFO agent operation phase=connect status=ok\nunfinished secret");
        std::fs::write(&path, bytes).unwrap();
        let snapshot = collect_logs_in(dir.path(), None, None).unwrap();
        assert_eq!(snapshot.entries.len(), 1);
        assert!(!snapshot.entries[0].1.contains("private"));
        assert!(!snapshot.entries[0].1.contains("unfinished"));
        assert!(snapshot.entries[0].1.contains("agent operation"));
        let coverage = &snapshot.coverage.files[0];
        assert!(coverage.omitted_prefix_bytes > 0);
        assert!(coverage.omitted_partial_suffix_bytes > 0);
        assert_eq!(coverage.omitted_legacy_frontend_lines, 1);
        assert_eq!(coverage.omitted_unstructured_lines, 1);
    }

    #[test]
    fn empty_and_short_log_lines_cannot_expand_the_export() {
        let raw = "\n".repeat(MAX_LOG_FILE_BYTES as usize);
        let (sanitized, exported, omitted) = sanitize_log_lines(&raw);
        assert!(sanitized.is_empty());
        assert_eq!(exported, 0);
        assert_eq!(omitted, 0);
        let line = "2026-01-01T00:00:00Z token=x\n";
        let raw = line.repeat(MAX_LOG_FILE_BYTES as usize / line.len());
        let (sanitized, exported, omitted) = sanitize_log_lines(&raw);
        assert!(sanitized.len() <= MAX_LOG_FILE_BYTES as usize);
        assert!(omitted > 0);
        assert_eq!(exported + omitted, raw.lines().count());
        assert!(!sanitized.contains("token=x"));
    }
}
