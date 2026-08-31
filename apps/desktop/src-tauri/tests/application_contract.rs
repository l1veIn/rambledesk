use rambledesk_core::{
    ApplicationError, ApplicationErrorCode, FeedbackPackageContent, FeedbackPackageManifest,
    FeedbackPackageView, FeedbackResolution, RepositoryError,
};

#[test]
fn application_error_codes_preserve_the_public_json_contract() {
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
    let serialized = ApplicationErrorCode::ALL.map(|code| {
        serde_json::to_value(code)
            .expect("application error code should serialize")
            .as_str()
            .expect("application error code should be a JSON string")
            .to_owned()
    });

    assert_eq!(serialized.as_slice(), expected);
}

#[test]
fn application_error_json_shape_matches_the_transport_contract() {
    let error = ApplicationError::from(RepositoryError::PackageRead);
    assert_eq!(
        serde_json::to_value(error).expect("application error should serialize"),
        serde_json::json!({
            "code": "FEEDBACK_PACKAGE_READ_FAILURE",
            "message": "feedback package could not be read or verified",
            "retryable": true,
        })
    );
}

#[test]
fn feedback_package_view_omits_storage_paths() {
    let view = FeedbackPackageView::from(FeedbackPackageContent {
        manifest: FeedbackPackageManifest {
            schema_version: 1,
            request_id: "request-1".into(),
            title: "Review".into(),
            host_id: "codex".into(),
            host_session_id: "session-1".into(),
            source_hint: None,
            submitted_at: "2026-09-01T00:00:00Z".into(),
            resolution: FeedbackResolution::FeedbackSubmitted,
            cancel_reason: None,
            source_revision: 2,
            draft_revision: 3,
            feedback_markdown: "feedback.md".into(),
            feedback_sha256: "feedback-sha".into(),
            uncooked_markdown: None,
            uncooked_sha256: None,
            cooking_model: None,
            attachments: vec![],
            request_attachments: vec![],
        },
        markdown: "## Operator Feedback".into(),
        uncooked_markdown: Some("Original".into()),
        attachment_paths: vec!["/private/storage/attachment.png".into()],
        request_attachment_paths: vec!["/private/storage/request.txt".into()],
    });

    let json = serde_json::to_value(view).expect("feedback package view should serialize");
    assert_eq!(json["markdown"], "## Operator Feedback");
    assert_eq!(json["uncooked_markdown"], "Original");
    assert!(json.get("attachment_paths").is_none());
    assert!(json.get("request_attachment_paths").is_none());
}
