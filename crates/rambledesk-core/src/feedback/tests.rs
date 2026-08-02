use super::validation::valid_action_id;
use super::*;

#[test]
fn validates_action_id_format() {
    assert!(valid_action_id("open-onboarding_1"));
    assert!(!valid_action_id("Open"));
    assert!(!valid_action_id("-leading"));
}

#[test]
fn application_errors_expose_stable_public_fields() {
    let error = ApplicationError::from(RepositoryError::RequestConflict);
    assert_eq!(error.code(), "REQUEST_CONFLICT");
    assert!(!error.retryable());
    assert!(!error.message().contains("sqlite"));
}

#[test]
fn canonicalizes_uuid_inputs() {
    let canonical = "0195f7e2-5c31-7b5a-8ab7-3c84ea4fc827";
    assert_eq!(
        canonical_uuid(&canonical.to_uppercase(), "request_id").expect("uppercase UUID"),
        canonical
    );
}

#[test]
fn terminal_results_omit_poll_interval() {
    let value = serde_json::to_value(FeedbackRequestView::from(StoredFeedbackRequest {
        request_id: "request".to_owned(),
        host_id: "generic".to_owned(),
        host_session_id: "session".to_owned(),
        status: FeedbackStatus::Cancelled,
        created_at: "2026-07-29T00:00:00Z".to_owned(),
        updated_at: "2026-07-29T00:01:00Z".to_owned(),
        feedback: None,
    }))
    .expect("feedback result");
    assert!(value.get("poll_after_ms").is_none());
}
