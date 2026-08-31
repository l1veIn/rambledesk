use rambledesk_core::kernel::{
    AccessMode, LaunchConfiguration, LaunchSubmission, RambleContent, SubmissionId,
};

use super::super::AcpWorkbenchState;
use super::super::model::{RenameAcpSessionInput, SetAcpSessionPinnedInput};

#[tokio::test]
async fn desktop_session_organization_keeps_active_ramble_visible_until_it_is_ended() {
    let temp = tempfile::tempdir().expect("temporary v3 root");
    let state = AcpWorkbenchState::open_unavailable(crate::config::v3_storage_paths(
        temp.path().join("target"),
    ))
    .await
    .expect("open ACP Workbench");
    let launched = launch_setup_session(&state, temp.path()).await;

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

    let error = state
        .archive_session(launched.session_id.to_string())
        .await
        .expect_err("an active managed Ramble must be ended before archive");
    assert_eq!(error.code, "SESSION_HAS_PENDING_ACTIVITY");
    let active = state.read().await.expect("read active Session");
    assert_eq!(active.sessions.len(), 1);
    assert_eq!(active.sessions[0].title, "Pinned project");
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
