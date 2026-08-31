use rambledesk_core::kernel::{
    AccessMode, CreateFeedbackRequest, FeedbackAction, FeedbackResolution, GetFeedback,
    GetFeedbackOutcome, LaunchConfiguration, LaunchSubmission, RambleContent, RequestId,
    SubmissionId, WorkbenchQuery,
};

use super::model::{AttentionItem, AttentionStatus, SessionStatus};
use super::*;

mod live_acceptance;
mod session_organization;

struct ReconcileFailsOrchestrator;

impl AcpOrchestrationPort for ReconcileFailsOrchestrator {
    fn live_projection(&self) -> LiveAcpProjection {
        LiveAcpProjection::default()
    }

    fn connect<'a>(
        &'a self,
        agent_id: &'a str,
    ) -> orchestration::OrchestrationFuture<'a, LaunchPreflight> {
        Box::pin(async move {
            Ok(LaunchPreflight {
                agent_id: agent_id.to_owned(),
                models: vec!["gpt-5".to_owned()],
                reasoning_efforts: vec!["high".to_owned()],
                access_modes: vec![AccessMode::WorkspaceWrite],
                warning: None,
            })
        })
    }

    fn preflight<'a>(
        &'a self,
        input: &'a LaunchDraftInput,
    ) -> orchestration::OrchestrationFuture<'a, LaunchPreflight> {
        Box::pin(async move {
            Ok(LaunchPreflight {
                agent_id: input.agent_id.clone(),
                models: vec![input.model.clone()],
                reasoning_efforts: vec![input.reasoning_effort.clone()],
                access_modes: vec![input.access_mode],
                warning: None,
            })
        })
    }

    fn reconcile<'a>(
        &'a self,
        _session_id: rambledesk_core::kernel::SessionId,
    ) -> orchestration::OrchestrationFuture<'a, ()> {
        Box::pin(async {
            Err(AcpWorkbenchError::new(
                "ACP_AGENT_START_FAILED",
                "the Agent rejected the selected workspace",
                true,
            ))
        })
    }

    fn answer_permission<'a>(
        &'a self,
        _input: PermissionAnswerInput,
    ) -> orchestration::OrchestrationFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn answer_question<'a>(
        &'a self,
        _input: QuestionAnswerInput,
    ) -> orchestration::OrchestrationFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown(&self) -> orchestration::OrchestrationFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn launch_input(workspace: impl Into<String>, submission_id: &str) -> LaunchDraftInput {
    LaunchDraftInput {
        submission_id: submission_id.to_owned(),
        workspace: workspace.into(),
        agent_id: "codex".to_owned(),
        model: "gpt-5".to_owned(),
        reasoning_effort: "high".to_owned(),
        access_mode: AccessMode::WorkspaceWrite,
        document_json: r#"{"type":"doc"}"#.to_owned(),
        body_markdown: "# Verify the Managed ACP Desktop wiring".to_owned(),
    }
}

#[tokio::test]
async fn launch_requires_a_real_acp_preflight_before_creating_a_session() {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_unavailable(crate::config::v3_storage_paths(
        temp.path().join("target"),
    ))
    .await
    .expect("open ACP Workbench");
    let input = launch_input(temp.path().to_string_lossy(), "desktop-launch-1");

    let error = state
        .launch(input)
        .await
        .expect_err("Launch must not fake an unavailable ACP Client");

    assert_eq!(error.code, "ACP_CLIENT_UNAVAILABLE");
    assert!(
        state
            .read()
            .await
            .expect("read Workbench")
            .sessions
            .is_empty()
    );
}

#[tokio::test]
async fn preflight_rejects_an_invalid_selected_workspace_before_agent_probe() {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_unavailable(crate::config::v3_storage_paths(
        temp.path().join("target"),
    ))
    .await
    .expect("open ACP Workbench");
    let missing_workspace = temp.path().join("does-not-exist");
    let input = launch_input(
        missing_workspace.to_string_lossy(),
        "desktop-invalid-preflight",
    );

    let error = state
        .preflight(&input)
        .await
        .expect_err("a missing selected workspace must fail before probing the Agent");

    assert_eq!(error.code, "INVALID_WORKSPACE");
    assert!(!error.retryable);
    assert!(!error.local_fact_committed);
}

#[tokio::test]
async fn client_connection_probe_does_not_require_a_workspace() {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_with_orchestration(
        crate::config::v3_storage_paths(temp.path().join("target")),
        Arc::new(ReconcileFailsOrchestrator),
    )
    .await
    .expect("open ACP Workbench");

    let report = state.check_client_readiness("codex".to_owned()).await;

    assert_eq!(report.agent_id, "codex");
    assert_eq!(report.status, AcpClientReadinessStatus::Ready);
    assert_eq!(report.reason, None);

    let wire = serde_json::to_value(report).expect("serialize readiness");
    let encoded = wire.to_string();
    for forbidden in ["command", "args", "env", "models", "configOptions"] {
        assert!(!encoded.contains(forbidden), "readiness leaked {forbidden}");
    }
}

#[tokio::test]
async fn launch_rejects_an_invalid_workspace_without_creating_a_session() {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_with_orchestration(
        crate::config::v3_storage_paths(temp.path().join("target")),
        Arc::new(ReconcileFailsOrchestrator),
    )
    .await
    .expect("open ACP Workbench");
    let workspace_file = temp.path().join("not-a-directory");
    std::fs::write(&workspace_file, b"not a workspace").expect("write workspace fixture");

    let error = state
        .launch(launch_input(
            workspace_file.to_string_lossy(),
            "desktop-invalid-launch",
        ))
        .await
        .expect_err("a file cannot be used as the selected workspace");

    assert_eq!(error.code, "INVALID_WORKSPACE");
    assert!(!error.local_fact_committed);
    assert!(
        state
            .read()
            .await
            .expect("read Workbench")
            .sessions
            .is_empty()
    );
}

#[tokio::test]
async fn launch_reports_agent_start_failure_after_preserving_the_offline_session() {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_with_orchestration(
        crate::config::v3_storage_paths(temp.path().join("target")),
        Arc::new(ReconcileFailsOrchestrator),
    )
    .await
    .expect("open ACP Workbench");

    let error = state
        .launch(launch_input(
            temp.path().to_string_lossy(),
            "desktop-post-commit-failure",
        ))
        .await
        .expect_err("an Agent start failure must not be reported as launched");

    assert_eq!(error.code, "ACP_AGENT_START_FAILED");
    assert!(error.retryable);
    assert!(error.local_fact_committed);
    assert!(error.message.contains("Offline Session"));
    let workbench = state.read().await.expect("read Workbench");
    assert_eq!(workbench.sessions.len(), 1);
    assert_eq!(workbench.sessions[0].status, SessionStatus::Offline);
    assert_eq!(
        workbench.sessions[0].workspace,
        temp.path().to_string_lossy()
    );
}

#[tokio::test]
async fn desktop_wire_contract_is_camel_case_and_unavailable_is_explicit() {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_unavailable(crate::config::v3_storage_paths(
        temp.path().join("target"),
    ))
    .await
    .expect("open ACP Workbench");
    let input: LaunchDraftInput = serde_json::from_value(serde_json::json!({
        "submissionId": "desktop-wire-1",
        "workspace": temp.path().to_string_lossy(),
        "agentId": "codex",
        "model": "gpt-5",
        "reasoningEffort": "high",
        "accessMode": "workspace_write",
        "documentJson": r#"{"type":"doc"}"#,
        "bodyMarkdown": "# Wire contract"
    }))
    .expect("deserialize the frontend launch payload");

    let unavailable = state
        .preflight(&input)
        .await
        .expect_err("the placeholder orchestration port must not fake success");
    assert_eq!(unavailable.code, "ACP_CLIENT_UNAVAILABLE");
    assert!(unavailable.retryable);
    assert!(!unavailable.local_fact_committed);

    let wire = serde_json::to_value(state.read().await.expect("read Workbench"))
        .expect("serialize Workbench snapshot");
    assert!(wire.get("attentionItems").is_some());
    assert!(wire.get("attention_items").is_none());
}

#[tokio::test]
async fn feedback_submission_commits_locally_before_acp_reconcile() {
    let (_temp, state, request_id) = workbench_with_waiting_feedback().await;
    let saved = state
        .save_draft(DraftInput {
            request_id: request_id.clone(),
            expected_revision: 0,
            document_json: r#"{"type":"doc","content":[]}"#.to_owned(),
            body_markdown: "Human feedback".to_owned(),
        })
        .await
        .expect("save Feedback Draft");
    assert_eq!(saved.attention_items.len(), 1);
    let draft = state
        .read_ramble_draft_detail(request_id.clone())
        .await
        .expect("read Ramble Draft detail")
        .expect("saved Ramble Draft");
    assert_eq!(draft.revision, 1);
    assert_eq!(draft.body_markdown, "Human feedback");

    let after_submit = state
        .submit_feedback(FeedbackDecisionInput {
            submission_id: "desktop-feedback-1".to_owned(),
            request_id: request_id.clone(),
            expected_revision: 1,
            document_json: r#"{"type":"doc","content":[]}"#.to_owned(),
            body_markdown: "Human feedback".to_owned(),
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .expect("submit while ACP Client is unavailable");

    assert!(matches!(
        after_submit.attention_items.as_slice(),
        [AttentionItem::Feedback {
            status: AttentionStatus::Submitted,
            ..
        }]
    ));
    assert_eq!(after_submit.sessions[0].status, SessionStatus::Offline);
    let detail = state
        .read_feedback_detail(request_id.clone())
        .await
        .expect("read submitted Feedback detail");
    assert_eq!(
        detail.request.status,
        rambledesk_core::kernel::FeedbackRequestStatus::Submitted
    );
    assert!(detail.delivery.is_some());
    assert_eq!(
        detail
            .published_feedback
            .as_ref()
            .map(|value| value.markdown.as_str()),
        Some("Human feedback")
    );
    let terminal = state
        .core
        .get_feedback(GetFeedback {
            request_id: RequestId::new(&request_id),
        })
        .await
        .expect("read terminal Feedback");
    let GetFeedbackOutcome::Terminal(envelope) = terminal else {
        panic!("Feedback must be terminal after local commit")
    };
    assert_eq!(envelope.resolution, FeedbackResolution::Submitted);
    assert_eq!(envelope.artifacts.len(), 2);
    let durable = state
        .core
        .read_workbench(WorkbenchQuery { session_id: None })
        .await
        .expect("read durable outbox");
    assert_eq!(durable.pending_deliveries.len(), 1);
}

#[tokio::test]
async fn feedback_cancellation_commits_locally_before_acp_reconcile() {
    let (_temp, state, request_id) = workbench_with_waiting_feedback().await;

    let after_cancel = state
        .cancel_feedback(request_id.clone())
        .await
        .expect("cancel while ACP Client is unavailable");

    assert!(matches!(
        after_cancel.attention_items.as_slice(),
        [AttentionItem::Feedback {
            status: AttentionStatus::Cancelled,
            ..
        }]
    ));
    let detail = state
        .read_feedback_detail(request_id.clone())
        .await
        .expect("read cancelled Feedback detail");
    assert_eq!(
        detail.request.status,
        rambledesk_core::kernel::FeedbackRequestStatus::Cancelled
    );
    assert!(detail.delivery.is_some());
    let terminal = state
        .core
        .get_feedback(GetFeedback {
            request_id: RequestId::new(request_id),
        })
        .await
        .expect("read cancelled Feedback");
    let GetFeedbackOutcome::Terminal(envelope) = terminal else {
        panic!("Feedback must be terminal after local cancellation")
    };
    assert_eq!(envelope.resolution, FeedbackResolution::Cancelled);
    assert!(envelope.package_id.is_none());
    assert_eq!(
        envelope.cancel_reason.as_deref(),
        Some("Cancelled by the human in RambleDesk.")
    );
}

#[tokio::test]
async fn voice_ramble_lookup_reports_an_absent_managed_acp_request_without_cross_source_lookup() {
    let (_temp, state, request_id) = workbench_with_waiting_feedback().await;

    assert_eq!(
        state
            .voice_feedback_status(&request_id)
            .await
            .expect("read waiting v3 Feedback status"),
        Some(rambledesk_core::kernel::FeedbackRequestStatus::Waiting)
    );
    assert_eq!(
        state
            .voice_feedback_status("legacy-only-request")
            .await
            .expect("missing Managed ACP Feedback remains explicit"),
        None
    );

    state
        .cancel_feedback(request_id.clone())
        .await
        .expect("close v3 Feedback Request");
    assert_eq!(
        state
            .voice_feedback_status(&request_id)
            .await
            .expect("read terminal v3 Feedback status"),
        Some(rambledesk_core::kernel::FeedbackRequestStatus::Cancelled)
    );
}

#[tokio::test]
async fn feedback_detail_includes_the_bound_draft_and_verified_artifact_bytes() {
    let (_temp, state, request_id) = workbench_with_waiting_feedback().await;
    state
        .save_draft(DraftInput {
            request_id: request_id.clone(),
            expected_revision: 0,
            document_json: r#"{"type":"doc","content":[]}"#.to_owned(),
            body_markdown: "Draft body".to_owned(),
        })
        .await
        .expect("save initial Feedback Draft");

    let draft = state
        .add_draft_artifact(AddDraftArtifactInput {
            request_id: request_id.clone(),
            expected_revision: 1,
            file_name: "evidence.txt".to_owned(),
            media_type: "text/plain".to_owned(),
            contents: b"verified evidence".to_vec(),
        })
        .await
        .expect("add Draft Artifact");
    let artifact_id = draft.artifacts[0].artifact_id.clone();

    let detail = state
        .read_feedback_detail(request_id.clone())
        .await
        .expect("read Feedback detail");
    let detail_draft = detail.draft.expect("Feedback detail includes its Draft");
    assert_eq!(detail_draft.draft_id, draft.draft_id);
    assert_eq!(
        detail_draft.request_id.as_deref(),
        Some(request_id.as_str())
    );
    assert_eq!(detail_draft.revision, 2);
    assert_eq!(detail_draft.document_json, r#"{"type":"doc","content":[]}"#);
    assert_eq!(detail_draft.body_markdown, "Draft body");
    assert_eq!(detail_draft.artifacts.len(), 1);
    assert_eq!(detail_draft.artifacts[0].file_name, "evidence.txt");
    assert_eq!(detail_draft.artifacts[0].media_type, "text/plain");
    assert_eq!(detail_draft.artifacts[0].byte_size, 17);
    assert_eq!(detail_draft.artifacts[0].position, 0);
    let wire = serde_json::to_value(&detail_draft).expect("serialize Draft projection");
    assert_eq!(wire["artifacts"][0]["artifactId"], artifact_id);
    assert_eq!(wire["artifacts"][0]["fileName"], "evidence.txt");
    assert!(wire["artifacts"][0].get("storageKey").is_none());
    assert!(wire["artifacts"][0].get("storage_key").is_none());

    let bytes = state
        .read_draft_artifact(request_id, artifact_id)
        .await
        .expect("read digest-verified Draft Artifact");
    assert_eq!(bytes, b"verified evidence");
}

#[tokio::test]
async fn draft_artifact_mutations_enforce_revision_and_preserve_explicit_order() {
    let (_temp, state, request_id) = workbench_with_waiting_feedback().await;
    state
        .save_draft(DraftInput {
            request_id: request_id.clone(),
            expected_revision: 0,
            document_json: r#"{"type":"doc"}"#.to_owned(),
            body_markdown: "Order artifacts".to_owned(),
        })
        .await
        .expect("save initial Feedback Draft");
    let first = state
        .add_draft_artifact(AddDraftArtifactInput {
            request_id: request_id.clone(),
            expected_revision: 1,
            file_name: "first.txt".to_owned(),
            media_type: "text/plain".to_owned(),
            contents: b"first".to_vec(),
        })
        .await
        .expect("add first artifact");
    let first_id = first.artifacts[0].artifact_id.clone();
    let second = state
        .add_draft_artifact(AddDraftArtifactInput {
            request_id: request_id.clone(),
            expected_revision: 2,
            file_name: "second.txt".to_owned(),
            media_type: "text/plain".to_owned(),
            contents: b"second".to_vec(),
        })
        .await
        .expect("add second artifact");
    let second_id = second.artifacts[1].artifact_id.clone();

    let conflict = state
        .remove_draft_artifact(RemoveDraftArtifactInput {
            request_id: request_id.clone(),
            artifact_id: first_id.clone(),
            expected_revision: 2,
        })
        .await
        .expect_err("stale mutation must not overwrite the latest Draft");
    assert_eq!(conflict.code, "DRAFT_CONFLICT");

    let reordered = state
        .reorder_draft_artifacts(ReorderDraftArtifactsInput {
            request_id: request_id.clone(),
            artifact_ids: vec![second_id.clone(), first_id.clone()],
            expected_revision: 3,
        })
        .await
        .expect("reorder Draft Artifacts");
    assert_eq!(reordered.revision, 4);
    assert_eq!(reordered.artifacts[0].artifact_id, second_id);
    assert_eq!(reordered.artifacts[0].position, 0);
    assert_eq!(reordered.artifacts[1].artifact_id, first_id);
    assert_eq!(reordered.artifacts[1].position, 1);

    let removed = state
        .remove_draft_artifact(RemoveDraftArtifactInput {
            request_id,
            artifact_id: first_id,
            expected_revision: 4,
        })
        .await
        .expect("remove Draft Artifact");
    assert_eq!(removed.revision, 5);
    assert_eq!(removed.artifacts.len(), 1);
    assert_eq!(removed.artifacts[0].artifact_id, second_id);
    assert_eq!(removed.artifacts[0].position, 0);
}

#[tokio::test]
async fn draft_artifact_access_rejects_another_request_and_a_terminal_request() {
    let (_temp, state, first_request_id) = workbench_with_waiting_feedback().await;
    let first_detail = state
        .read_feedback_detail(first_request_id.clone())
        .await
        .expect("read first Feedback Request");
    let second_request_id = "desktop-request-2".to_owned();
    state
        .core
        .request_feedback(CreateFeedbackRequest {
            request_id: Some(RequestId::new(&second_request_id)),
            session_id: first_detail.request.session_id,
            source_link_id: None,
            title: "Second review".to_owned(),
            instructions: "Keep request ownership isolated.".to_owned(),
            actions: vec![FeedbackAction {
                id: "review".to_owned(),
                instruction: "Review the second request".to_owned(),
            }],
            context_refs: Vec::new(),
            artifacts: Vec::new(),
        })
        .await
        .expect("create second Feedback Request");
    for request_id in [&first_request_id, &second_request_id] {
        state
            .save_draft(DraftInput {
                request_id: request_id.clone(),
                expected_revision: 0,
                document_json: r#"{"type":"doc"}"#.to_owned(),
                body_markdown: "Owned draft".to_owned(),
            })
            .await
            .expect("save owned Feedback Draft");
    }
    let first_draft = state
        .add_draft_artifact(AddDraftArtifactInput {
            request_id: first_request_id.clone(),
            expected_revision: 1,
            file_name: "private.txt".to_owned(),
            media_type: "text/plain".to_owned(),
            contents: b"request scoped".to_vec(),
        })
        .await
        .expect("add first request artifact");
    let artifact_id = first_draft.artifacts[0].artifact_id.clone();

    let cross_request = state
        .read_draft_artifact(second_request_id.clone(), artifact_id.clone())
        .await
        .expect_err("another request must not read this artifact");
    assert_eq!(cross_request.code, "ARTIFACT_NOT_FOUND");
    let cross_request = state
        .remove_draft_artifact(RemoveDraftArtifactInput {
            request_id: second_request_id,
            artifact_id: artifact_id.clone(),
            expected_revision: 1,
        })
        .await
        .expect_err("another request must not mutate this artifact");
    assert_eq!(cross_request.code, "ARTIFACT_NOT_FOUND");

    state
        .cancel_feedback(first_request_id.clone())
        .await
        .expect("close first Feedback Request");
    let terminal = state
        .add_draft_artifact(AddDraftArtifactInput {
            request_id: first_request_id.clone(),
            expected_revision: 2,
            file_name: "too-late.txt".to_owned(),
            media_type: "text/plain".to_owned(),
            contents: b"too late".to_vec(),
        })
        .await
        .expect_err("a terminal request cannot accept Draft Artifacts");
    assert_eq!(terminal.code, "REQUEST_TERMINAL");
    let terminal = state
        .read_draft_artifact(first_request_id.clone(), artifact_id)
        .await
        .expect_err("a terminal request cannot expose its deleted Draft");
    assert_eq!(terminal.code, "REQUEST_TERMINAL");
    assert!(
        state
            .read_feedback_detail(first_request_id)
            .await
            .expect("read terminal Feedback detail")
            .draft
            .is_none()
    );
}

async fn workbench_with_waiting_feedback() -> (tempfile::TempDir, AcpWorkbenchState, String) {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_unavailable(crate::config::v3_storage_paths(
        temp.path().join("target"),
    ))
    .await
    .expect("open ACP Workbench");
    let launched = state
        .core
        .launch(LaunchSubmission {
            submission_id: SubmissionId::new("feedback-setup-launch"),
            submission_digest_assertion: None,
            title: "Prepare Feedback Request".to_owned(),
            launch_configuration: LaunchConfiguration {
                agent_profile_id: "codex".to_owned(),
                launch_profile_id: "codex-acp-npx".to_owned(),
                workspace_reference: temp.path().to_string_lossy().into_owned(),
                model: Some("gpt-5".to_owned()),
                reasoning_effort: Some("high".to_owned()),
                access_mode: AccessMode::WorkspaceWrite,
                agent_config_json: "{}".to_owned(),
            },
            ramble: RambleContent {
                document_json: r#"{"type":"doc"}"#.to_owned(),
                body_markdown: "Prepare Feedback Request".to_owned(),
                artifacts: Vec::new(),
            },
        })
        .await
        .expect("create durable setup Session");
    let request_id = "desktop-request-1".to_owned();
    state
        .core
        .request_feedback(CreateFeedbackRequest {
            request_id: Some(RequestId::new(&request_id)),
            session_id: launched.session_id,
            source_link_id: None,
            title: "Review the result".to_owned(),
            instructions: "Exercise the result and report what happened.".to_owned(),
            actions: vec![FeedbackAction {
                id: "review".to_owned(),
                instruction: "Review the result".to_owned(),
            }],
            context_refs: Vec::new(),
            artifacts: Vec::new(),
        })
        .await
        .expect("create durable Feedback Request");
    (temp, state, request_id)
}
