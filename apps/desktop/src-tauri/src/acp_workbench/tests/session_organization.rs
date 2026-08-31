use rambledesk_core::kernel::{
    AccessMode, AgentWorkDisposition, AgentWorkEvidence, AgentWorkResult, LaunchConfiguration,
    LaunchSubmission, RambleContent, SubmissionId, WorkScope,
};

use super::super::AcpWorkbenchState;
use super::super::model::{RenameAcpSessionInput, SetAcpSessionPinnedInput};

#[tokio::test]
async fn desktop_session_organization_projects_active_archive_and_restore() {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_unavailable(crate::config::v3_storage_paths(
        temp.path().join("target"),
    ))
    .await
    .expect("open ACP Workbench");
    let launched = launch_setup_session(&state, temp.path()).await;
    complete_launch_work(&state, launched.session_id.clone()).await;

    let renamed = state
        .rename_session(RenameAcpSessionInput {
            session_id: launched.session_id.to_string(),
            title: "Pinned project".to_owned(),
        })
        .await
        .expect("rename Session");
    assert_eq!(renamed.sessions[0].title, "Pinned project");
    let pinned = state
        .set_session_pinned(SetAcpSessionPinnedInput {
            session_id: launched.session_id.to_string(),
            pinned: true,
        })
        .await
        .expect("pin Session");
    assert!(pinned.sessions[0].pinned_at.is_some());
    assert!(pinned.sessions[0].archived_at.is_none());

    let active = state
        .archive_session(launched.session_id.to_string())
        .await
        .expect("archive idle Session");
    assert!(active.sessions.is_empty());
    let archived = state
        .read_archived_sessions()
        .await
        .expect("read archived Sessions");
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].title, "Pinned project");
    assert!(archived[0].pinned_at.is_some());
    assert!(archived[0].archived_at.is_some());

    let restored = state
        .unarchive_session(launched.session_id.to_string())
        .await
        .expect("restore archived Session");
    assert_eq!(restored.sessions.len(), 1);
    assert!(restored.sessions[0].archived_at.is_none());
}

async fn launch_setup_session(
    state: &AcpWorkbenchState,
    workspace: &std::path::Path,
) -> rambledesk_core::kernel::LaunchOutcome {
    state
        .core
        .launch(LaunchSubmission {
            submission_id: SubmissionId::new(format!("organization-{}", uuid::Uuid::now_v7())),
            submission_digest_assertion: None,
            title: "Organize Session".to_owned(),
            launch_configuration: LaunchConfiguration {
                agent_profile_id: "codex".to_owned(),
                launch_profile_id: "codex-acp-npx".to_owned(),
                workspace_reference: workspace.to_string_lossy().into_owned(),
                model: Some("gpt-5".to_owned()),
                reasoning_effort: Some("high".to_owned()),
                access_mode: AccessMode::WorkspaceWrite,
                agent_config_json: "{}".to_owned(),
            },
            ramble: RambleContent {
                document_json: r#"{"type":"doc"}"#.to_owned(),
                body_markdown: "Organize Session".to_owned(),
                artifacts: Vec::new(),
            },
        })
        .await
        .expect("create durable Session")
}

async fn complete_launch_work(
    state: &AcpWorkbenchState,
    session_id: rambledesk_core::kernel::SessionId,
) {
    let batch = state
        .core
        .claim_agent_work(WorkScope {
            session_id: Some(session_id),
            limit: 1,
            lease_seconds: 60,
        })
        .await
        .expect("claim Launch work");
    state
        .core
        .record_agent_work(AgentWorkResult {
            work_id: batch.items[0].work.work_id.clone(),
            claim_token: batch.items[0].claim_token.clone(),
            disposition: AgentWorkDisposition::Completed {
                evidence: AgentWorkEvidence::PromptTurnCompleted,
            },
        })
        .await
        .expect("complete Launch work");
}
