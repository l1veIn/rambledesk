use std::sync::Arc;

use super::{
    AccessMode, ContextReference, Core, CreateFeedbackRequest, DraftMutation, DraftSnapshot,
    FeedbackAction, FeedbackSubmission, LaunchConfiguration, LaunchOutcome, LaunchSubmission,
    RambleContent, RambleIntent, RequestId, SaveDraft, SessionId, SubmissionId,
    digest::{feedback_submission_digest, launch_submission_digest},
    test_adapters::memory_artifact_store,
    tests::MemoryFactStore,
};

pub(super) fn harness() -> (Core, Arc<MemoryFactStore>) {
    let facts = Arc::new(MemoryFactStore::default());
    let artifacts = memory_artifact_store();
    (Core::new(facts.clone(), artifacts), facts)
}

pub(super) fn launch_input(id: &str, body: &str) -> LaunchSubmission {
    let mut input = LaunchSubmission {
        submission_id: SubmissionId::from(id),
        submission_digest_assertion: None,
        title: "Managed ACP session".to_owned(),
        launch_configuration: LaunchConfiguration {
            agent_profile_id: "codex".to_owned(),
            launch_profile_id: "codex-acp-local".to_owned(),
            workspace_reference: "/workspace".to_owned(),
            model: Some("gpt-5".to_owned()),
            reasoning_effort: Some("high".to_owned()),
            access_mode: AccessMode::WorkspaceWrite,
            agent_config_json: "{}".to_owned(),
        },
        ramble: RambleContent {
            document_json: r#"{"type":"doc"}"#.to_owned(),
            body_markdown: body.to_owned(),
            artifacts: Vec::new(),
        },
    };
    input.submission_digest_assertion = Some(launch_submission_digest(&input));
    input
}

pub(super) async fn launch_session(core: &Core) -> LaunchOutcome {
    core.launch(launch_input("launch-1", "Build the Managed ACP path."))
        .await
        .expect("launch")
}

pub(super) async fn create_request(
    core: &Core,
    session_id: SessionId,
    id: &str,
) -> super::FeedbackRequestSnapshot {
    core.request_feedback(CreateFeedbackRequest {
        request_id: Some(RequestId::from(id)),
        session_id,
        source_link_id: None,
        title: "Review launch".to_owned(),
        instructions: "Judge the real launch flow.".to_owned(),
        actions: vec![FeedbackAction {
            id: "launch".to_owned(),
            instruction: "Launch Codex.".to_owned(),
        }],
        context_refs: vec![ContextReference {
            label: "Acceptance".to_owned(),
            uri: "rambledesk-context://acceptance".to_owned(),
        }],
        artifacts: Vec::new(),
    })
    .await
    .expect("request feedback")
}

pub(super) async fn save_feedback_draft(
    core: &Core,
    session_id: SessionId,
    request_id: RequestId,
) -> DraftSnapshot {
    core.mutate_draft(DraftMutation::Save(SaveDraft {
        draft_id: super::DraftId::from("draft-1"),
        intent: RambleIntent::Feedback,
        session_id: Some(session_id),
        request_id: Some(request_id),
        launch_configuration: None,
        document_json: r#"{"type":"doc","content":[]}"#.to_owned(),
        body_markdown: "Human feedback".to_owned(),
        expected_revision: 0,
    }))
    .await
    .expect("save draft")
}

pub(super) fn feedback_submission(request_id: RequestId, revision: u64) -> FeedbackSubmission {
    let mut input = FeedbackSubmission {
        submission_id: SubmissionId::from("feedback-1"),
        request_id,
        expected_draft_revision: revision,
        submission_digest_assertion: None,
        document_json: r#"{"type":"doc","content":[]}"#.to_owned(),
        uncooked_markdown: "Raw human feedback".to_owned(),
        feedback_markdown: "Structured human feedback".to_owned(),
        cooking_model: Some("model".to_owned()),
        artifacts: Vec::new(),
    };
    input.submission_digest_assertion = Some(feedback_submission_digest(&input));
    input
}
