use super::validation::{valid_action_id, validate_request_input};
use super::*;

#[test]
fn rejects_attachments_without_exactly_one_source() {
    let mut input = RequestFeedbackInput {
        request_id: None,
        host_id: Some("generic".to_owned()),
        host_session_id: "session".to_owned(),
        title: Some("Review".to_owned()),
        what_happened: "Need a screenshot review.".to_owned(),
        actions: vec![ActionInput {
            id: "look".to_owned(),
            instruction: "Look at the screenshot.".to_owned(),
        }],
        context_refs: Vec::new(),
        attachments: vec![RequestAttachmentInput {
            file_name: "shot.png".to_owned(),
            markdown: None,
            contents_base64: None,
            path: None,
        }],
        source_hint: None,
        allow_finish: false,
        final_summary: None,
    };
    assert!(validate_request_input(&input).is_err());
    input.attachments[0].path = Some("/tmp/shot.png".to_owned());
    input.attachments[0].contents_base64 = Some("aaaa".to_owned());
    assert!(validate_request_input(&input).is_err());
    input.attachments[0].contents_base64 = None;
    assert!(validate_request_input(&input).is_ok());
}

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
fn application_error_codes_preserve_the_public_code_contract() {
    let expected = [
        "INVALID_ARGUMENT",
        "REQUEST_NOT_FOUND",
        "RECOVERY_AMBIGUOUS",
        "REQUEST_CONFLICT",
        "REQUEST_ALREADY_COMPLETED",
        "REQUEST_TERMINAL",
        "DRAFT_CONFLICT",
        "ATTACHMENT_NOT_FOUND",
        "ATTACHMENT_LIMIT",
        "HOST_SESSION_NOT_FOUND",
        "HOST_SESSION_HAS_OPEN_REQUESTS",
        "DELETE_REQUIRES_ARCHIVED_HOST_SESSION",
        "REQUEST_NOT_TERMINAL",
        "PACKAGE_PUBLISH_FAILURE",
        "FEEDBACK_PACKAGE_READ_FAILURE",
        "STORAGE_FAILURE",
    ];
    for (code, expected) in ApplicationErrorCode::ALL.into_iter().zip(expected) {
        assert_eq!(code.as_str(), expected);
    }
    assert_eq!(
        ApplicationError::invalid_argument("invalid").code_enum(),
        ApplicationErrorCode::InvalidArgument
    );
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
    let value = FeedbackRequestView::from(StoredFeedbackRequest {
        request_id: "request".to_owned(),
        managed_session_id: None,
        host_id: "generic".to_owned(),
        host_session_id: "session".to_owned(),
        status: FeedbackStatus::Cancelled,
        created_at: "2026-07-29T00:00:00Z".to_owned(),
        updated_at: "2026-07-29T00:01:00Z".to_owned(),
        feedback: None,
        resolution: Some(FeedbackResolution::Cancelled),
        allow_finish: false,
        final_summary: None,
    });
    assert!(value.poll_after_ms.is_none());
}
