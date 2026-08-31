use rambledesk_core::kernel::{AccessMode, SessionLifecycle, SessionOrganization, SessionRecord};

use super::{
    AcpWorkbenchState,
    model::{
        AcpSessionSummary, AcpWorkbenchError, AcpWorkbenchSnapshot, AgentSummary,
        RenameAcpSessionInput, SessionStatus, SetAcpSessionPinnedInput,
    },
};

impl AcpWorkbenchState {
    pub(super) async fn rename_session(
        &self,
        input: RenameAcpSessionInput,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        self.core
            .organize_session(SessionOrganization::Rename {
                session_id: input.session_id.into(),
                title: input.title,
            })
            .await?;
        self.read().await
    }

    pub(super) async fn set_session_pinned(
        &self,
        input: SetAcpSessionPinnedInput,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        self.core
            .organize_session(SessionOrganization::SetPinned {
                session_id: input.session_id.into(),
                pinned: input.pinned,
            })
            .await?;
        self.read().await
    }

    pub(super) async fn archive_session(
        &self,
        session_id: String,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        if self
            .orchestration
            .live_projection()
            .attention_items
            .iter()
            .any(|item| item.session_id() == session_id && item.is_waiting())
        {
            return Err(AcpWorkbenchError::new(
                "SESSION_HAS_PENDING_ACTIVITY",
                "the Session has live attention that must be resolved first",
                false,
            ));
        }
        self.core
            .organize_session(SessionOrganization::SetArchived {
                session_id: session_id.into(),
                archived: true,
            })
            .await?;
        self.read().await
    }

    pub(super) async fn unarchive_session(
        &self,
        session_id: String,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        self.core
            .organize_session(SessionOrganization::SetArchived {
                session_id: session_id.into(),
                archived: false,
            })
            .await?;
        self.read().await
    }

    pub(super) async fn read_archived_sessions(
        &self,
    ) -> Result<Vec<AcpSessionSummary>, AcpWorkbenchError> {
        let sessions = self.core.read_archived_sessions().await?;
        Ok(project_archived_sessions(
            sessions,
            &self.runtime_catalog.agents(),
        ))
    }
}

fn project_archived_sessions(
    sessions: Vec<SessionRecord>,
    agents: &[AgentSummary],
) -> Vec<AcpSessionSummary> {
    sessions
        .into_iter()
        .map(|session| {
            let launch = session.launch_configuration.as_ref();
            let agent_id = launch
                .map(|configuration| configuration.agent_profile_id.clone())
                .unwrap_or_default();
            let agent_label = agents
                .iter()
                .find(|agent| agent.id == agent_id)
                .map(|agent| agent.label.clone())
                .unwrap_or_else(|| agent_id.clone());
            AcpSessionSummary {
                session_id: session.session_id.to_string(),
                title: session.title,
                agent_id,
                agent_label,
                workspace: launch
                    .map(|configuration| configuration.workspace_reference.clone())
                    .unwrap_or_default(),
                model: launch
                    .and_then(|configuration| configuration.model.clone())
                    .unwrap_or_default(),
                reasoning_effort: launch
                    .and_then(|configuration| configuration.reasoning_effort.clone())
                    .unwrap_or_default(),
                access_mode: launch
                    .map(|configuration| configuration.access_mode)
                    .unwrap_or(AccessMode::ReadOnly),
                status: if session.lifecycle == SessionLifecycle::Stopped {
                    SessionStatus::Completed
                } else {
                    SessionStatus::Offline
                },
                pending_count: 0,
                pinned_at: session.pinned_at,
                archived_at: session.archived_at,
                updated_at: session.updated_at,
            }
        })
        .collect()
}

#[tauri::command]
pub(crate) async fn rename_acp_session_v3(
    input: RenameAcpSessionInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.rename_session(input).await
}

#[tauri::command]
pub(crate) async fn set_acp_session_pinned_v3(
    input: SetAcpSessionPinnedInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.set_session_pinned(input).await
}

#[tauri::command]
pub(crate) async fn archive_acp_session_v3(
    session_id: String,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.archive_session(session_id).await
}

#[tauri::command]
pub(crate) async fn unarchive_acp_session_v3(
    session_id: String,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.unarchive_session(session_id).await
}

#[tauri::command]
pub(crate) async fn read_archived_acp_sessions_v3(
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<Vec<AcpSessionSummary>, AcpWorkbenchError> {
    state.read_archived_sessions().await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use rambledesk_core::kernel::{
        LaunchConfiguration, LaunchSubmission, RambleContent, SubmissionId,
    };

    use super::*;
    use crate::acp_workbench::{
        model::{AttentionItem, AttentionStatus, LaunchPreflight, LaunchPreflightInput},
        orchestration::{AcpOrchestrationPort, LiveAcpProjection, OrchestrationFuture},
    };

    #[derive(Default)]
    struct LiveAttentionOrchestrator(Mutex<LiveAcpProjection>);

    impl AcpOrchestrationPort for LiveAttentionOrchestrator {
        fn live_projection(&self) -> LiveAcpProjection {
            self.0.lock().expect("live projection").clone()
        }

        fn connect<'a>(&'a self, _agent_id: &'a str) -> OrchestrationFuture<'a, LaunchPreflight> {
            Box::pin(async { unreachable!("not used by this test") })
        }

        fn preflight<'a>(
            &'a self,
            _input: &'a LaunchPreflightInput,
        ) -> OrchestrationFuture<'a, LaunchPreflight> {
            Box::pin(async { unreachable!("not used by this test") })
        }

        fn reconcile<'a>(
            &'a self,
            _session_id: rambledesk_core::kernel::SessionId,
        ) -> OrchestrationFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }

        fn answer_permission<'a>(
            &'a self,
            _input: crate::acp_workbench::model::PermissionAnswerInput,
        ) -> OrchestrationFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }

        fn answer_question<'a>(
            &'a self,
            _input: crate::acp_workbench::model::QuestionAnswerInput,
        ) -> OrchestrationFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }

        fn shutdown(&self) -> OrchestrationFuture<'_, ()> {
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test]
    async fn archive_rejects_live_attention_without_hiding_the_session() {
        let temp = tempfile::tempdir().expect("temporary v3 root");
        let orchestration = std::sync::Arc::new(LiveAttentionOrchestrator::default());
        let state = AcpWorkbenchState::open_with_orchestration(
            crate::config::v3_storage_paths(temp.path().join("target")),
            orchestration.clone(),
        )
        .await
        .expect("open ACP Workbench");
        let launched = state
            .core
            .launch(LaunchSubmission {
                submission_id: SubmissionId::new("desktop-live-attention"),
                submission_digest_assertion: None,
                title: "Live attention".to_owned(),
                launch_configuration: LaunchConfiguration {
                    agent_profile_id: "codex".to_owned(),
                    launch_profile_id: "codex-acp-npx".to_owned(),
                    workspace_reference: temp.path().to_string_lossy().into_owned(),
                    model: None,
                    reasoning_effort: None,
                    access_mode: AccessMode::WorkspaceWrite,
                    agent_config_json: "{}".to_owned(),
                },
                ramble: RambleContent {
                    document_json: r#"{"type":"doc"}"#.to_owned(),
                    body_markdown: "Live attention".to_owned(),
                    artifacts: Vec::new(),
                },
            })
            .await
            .expect("launch Session");
        orchestration
            .0
            .lock()
            .expect("live projection")
            .attention_items
            .push(AttentionItem::Question {
                id: "question-1".to_owned(),
                session_id: launched.session_id.to_string(),
                title: "Choose".to_owned(),
                created_at: "2026-08-31T00:00:00Z".to_owned(),
                status: AttentionStatus::Waiting,
                prompt: "Choose one".to_owned(),
                choices: Vec::new(),
                multiple: false,
                allow_skip: true,
                unsupported_reason: None,
            });

        let error = state
            .archive_session(launched.session_id.to_string())
            .await
            .expect_err("live attention must block archive");
        assert_eq!(error.code, "SESSION_HAS_PENDING_ACTIVITY");
        assert_eq!(state.read().await.expect("read active").sessions.len(), 1);
        assert!(
            state
                .read_archived_sessions()
                .await
                .expect("archived")
                .is_empty()
        );
    }
}
