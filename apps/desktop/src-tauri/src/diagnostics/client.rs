//! The frontend and native boundary share one finite, content-free vocabulary.
use std::collections::BTreeMap;
use std::sync::OnceLock;

const MAX_DURATION_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(serde::Deserialize)]
struct Schema {
    activities: Vec<String>,
    outcomes: Vec<String>,
    details: BTreeMap<String, Vec<String>>,
    counts: Vec<String>,
    flags: Vec<String>,
}
fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/lib/diagnostics/clientDiagnosticSchema.json"
        ))
        .expect("bundled diagnostic schema")
    })
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientDiagnosticInput {
    activity: String,
    outcome: String,
    operation_id: String,
    duration_ms: Option<u64>,
    #[serde(default)]
    details: BTreeMap<String, serde_json::Value>,
}
pub fn valid_detail(key: &str, value: &serde_json::Value) -> bool {
    let schema = schema();
    if let Some(allowed) = schema.details.get(key) {
        return value
            .as_str()
            .is_some_and(|text| allowed.iter().any(|value| value == text));
    }
    if schema.counts.iter().any(|value| value == key) {
        return value
            .as_u64()
            .is_some_and(|value| value <= MAX_SAFE_INTEGER);
    }
    schema.flags.iter().any(|value| value == key) && value.is_boolean()
}
fn validate(input: &ClientDiagnosticInput) -> Result<(), &'static str> {
    let schema = schema();
    if !schema.activities.contains(&input.activity)
        || !schema.outcomes.contains(&input.outcome)
        || input.operation_id.len() != 36
        || uuid::Uuid::parse_str(&input.operation_id).is_err()
        || input
            .duration_ms
            .is_some_and(|value| value > MAX_DURATION_MS)
        || input.details.len() > 32
        || input
            .details
            .iter()
            .any(|(key, value)| !valid_detail(key, value))
    {
        return Err("invalid_client_diagnostic");
    }
    Ok(())
}
pub fn record(input: ClientDiagnosticInput) -> Result<(), String> {
    validate(&input)?;
    let mut event = super::events::new_event(
        &input.activity,
        None,
        None,
        Some(&input.outcome),
        input
            .details
            .get("error_category")
            .and_then(serde_json::Value::as_str),
        input.duration_ms,
    );
    event.operation_id = Some(input.operation_id);
    event.details = input.details;
    // The JS recorder absorbs this failure and counts dropped evidence. Returning
    // success here would silently hide disk failures from the next diagnostic ZIP.
    let Ok(control) = super::control::global() else {
        return Ok(());
    };
    persist_client_event(control, &super::events::events_path(), &event)
}

fn persist_client_event(
    control: &super::control::DiagnosticsControl,
    path: &std::path::Path,
    event: &super::events::ProcessEvent,
) -> Result<(), String> {
    super::events::record_if_enabled(control, path, event)
        .map_err(|_| "diagnostic_write_failed".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> serde_json::Value {
        serde_json::json!({"activity":"onboarding_action","outcome":"failed","operationId":"00000000-0000-4000-8000-000000000000","durationMs":14,"details":{"step":"voice","action":"download","error_category":"unknown","model_count":2,"installed":false}})
    }
    #[test]
    fn accepts_only_shared_vocabulary_and_bounded_metadata() {
        let parsed: ClientDiagnosticInput = serde_json::from_value(input()).unwrap();
        assert!(validate(&parsed).is_ok());
        for (key, value) in [
            ("prompt", serde_json::json!("private")),
            ("step", serde_json::json!("secret")),
            ("model_count", serde_json::json!(-1)),
            ("installed", serde_json::json!("false")),
            ("model_count", serde_json::json!(MAX_SAFE_INTEGER + 1)),
        ] {
            let mut invalid = input();
            invalid["details"][key] = value;
            assert_eq!(
                validate(&serde_json::from_value(invalid).unwrap()),
                Err("invalid_client_diagnostic")
            );
        }
    }
    #[test]
    fn rejects_free_text_identity_extra_fields_and_unbounded_duration() {
        for (key, value) in [
            ("activity", serde_json::json!("task title")),
            ("operationId", serde_json::json!("C:/private")),
            ("durationMs", serde_json::json!(MAX_DURATION_MS + 1)),
            ("outcome", serde_json::json!("raw error")),
        ] {
            let mut invalid = input();
            invalid[key] = value;
            assert!(validate(&serde_json::from_value(invalid).unwrap()).is_err());
        }
        let mut invalid = input();
        invalid["message"] = "secret text".into();
        assert!(serde_json::from_value::<ClientDiagnosticInput>(invalid).is_err());
    }

    #[test]
    fn persistence_failure_is_reported_without_exposing_a_filesystem_error() {
        let dir = tempfile::tempdir().unwrap();
        let control = super::super::control::DiagnosticsControl::load(dir.path().to_owned());
        let event =
            super::super::events::new_event("onboarding", None, None, Some("started"), None, None);
        // A directory cannot be opened as the event file on supported platforms.
        assert_eq!(
            persist_client_event(&control, dir.path(), &event),
            Err("diagnostic_write_failed".into())
        );
        let path = dir.path().join("events.jsonl");
        assert!(persist_client_event(&control, &path, &event).is_ok());
        control.set_enabled(false).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(persist_client_event(&control, &path, &event).is_ok());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
