mod artifacts;
mod model;
mod orchestration;
mod projection;
mod sessions;
mod settings;
mod validation;

pub(super) use artifacts::{
    add_completed_clipboard_capture_v3, add_completed_screen_capture_v3,
    import_feedback_draft_artifact_path_v3,
};
pub(super) use sessions::{
    archive_acp_session_v3, read_archived_acp_sessions_v3, rename_acp_session_v3,
    set_acp_session_pinned_v3, unarchive_acp_session_v3,
};

use std::{collections::HashSet, sync::Arc};

use rambledesk_core::kernel::{
    AccessMode, ArtifactInput, ArtifactRole, CancelFeedbackRequest, Core, DraftId, DraftMutation,
    DraftSnapshot, FeedbackDeliveryRecord, FeedbackRequestStatus, FeedbackSubmission,
    LaunchConfiguration, LaunchSubmission, PackageArtifact, RambleContent, RambleIntent, RequestId,
    ResolveFeedbackRequest, SaveDraft, SubmissionId, WorkbenchQuery, ports::ArtifactStore,
};
use rambledesk_storage::v3::{SqliteV3Store, V3FeedbackDetail, artifact::LocalArtifactStore};

use crate::config::V3StoragePaths;

use model::{
    AcpClientReadiness, AcpClientReadinessStatus, AcpWorkbenchError, AcpWorkbenchSnapshot,
    AddDraftArtifactInput, DraftInput, DraftSnapshotView, FeedbackDecisionInput,
    FeedbackDetailView, LaunchDraftInput, LaunchPreflight, LaunchPreflightInput,
    PermissionAnswerInput, PublishedFeedbackView, QuestionAnswerInput, RemoveDraftArtifactInput,
    ReorderDraftArtifactsInput,
};
#[cfg(test)]
use orchestration::LiveAcpProjection;
#[cfg(test)]
use orchestration::UnavailableAcpOrchestrator;
use orchestration::{AcpClientOrchestrator, AcpOrchestrationPort};
use projection::project_workbench;
use settings::AcpRuntimeCatalog;
use validation::{
    require_nonblank, title_from_markdown, validate_launch_selection, validate_selected_workspace,
};

pub(super) struct AcpWorkbenchState {
    core: Arc<Core>,
    facts: SqliteV3Store,
    artifacts: LocalArtifactStore,
    orchestration: Arc<dyn AcpOrchestrationPort>,
    runtime_catalog: AcpRuntimeCatalog,
}

impl AcpWorkbenchState {
    pub(super) async fn open(paths: V3StoragePaths) -> Result<Self, String> {
        let (core, facts, artifacts, runtime_catalog) = Self::open_parts(paths).await?;
        let orchestration = Arc::new(
            AcpClientOrchestrator::new(
                core.clone(),
                runtime_catalog.runtime_profiles(),
                runtime_catalog.agents(),
            )
            .map_err(|error| error.message)?,
        );
        let state = Self {
            core,
            facts,
            artifacts,
            orchestration,
            runtime_catalog,
        };
        state.start_pending_reconciliation().await?;
        Ok(state)
    }

    #[cfg(test)]
    async fn open_with_orchestration(
        paths: V3StoragePaths,
        orchestration: Arc<dyn AcpOrchestrationPort>,
    ) -> Result<Self, String> {
        let (core, facts, artifacts, runtime_catalog) = Self::open_parts(paths).await?;
        Ok(Self {
            core,
            facts,
            artifacts,
            orchestration,
            runtime_catalog,
        })
    }

    #[cfg(test)]
    async fn open_unavailable(paths: V3StoragePaths) -> Result<Self, String> {
        Self::open_with_orchestration(paths, Arc::new(UnavailableAcpOrchestrator)).await
    }

    async fn open_parts(
        paths: V3StoragePaths,
    ) -> Result<
        (
            Arc<Core>,
            SqliteV3Store,
            LocalArtifactStore,
            AcpRuntimeCatalog,
        ),
        String,
    > {
        let facts = SqliteV3Store::connect(&paths.database)
            .await
            .map_err(|error| error.to_string())?;
        let artifacts = LocalArtifactStore::open(&paths.library)
            .await
            .map_err(|error| error.to_string())?;
        let runtime_catalog =
            AcpRuntimeCatalog::open(&paths.root).map_err(|error| error.message)?;
        let core = Arc::new(Core::new(
            Arc::new(facts.clone()),
            Arc::new(artifacts.clone()),
        ));
        Ok((core, facts, artifacts, runtime_catalog))
    }

    async fn read(&self) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        let durable = self
            .core
            .read_workbench(WorkbenchQuery { session_id: None })
            .await?;
        let mut live = self.orchestration.live_projection();
        if live.agents.is_empty() {
            live.agents = self.runtime_catalog.agents();
        }
        Ok(project_workbench(durable, live))
    }

    async fn start_pending_reconciliation(&self) -> Result<(), String> {
        let durable = self
            .core
            .read_workbench(WorkbenchQuery { session_id: None })
            .await
            .map_err(|error| error.to_string())?;
        let mut sessions = HashSet::new();
        for work in durable.pending_agent_work {
            if !sessions.insert(work.session_id.to_string()) {
                continue;
            }
            let session_id = work.session_id;
            let orchestration = self.orchestration.clone();
            tokio::spawn(async move {
                if let Err(error) = orchestration.reconcile(session_id.clone()).await {
                    tracing::warn!(
                        code = %error.code,
                        %session_id,
                        "ACP startup reconciliation deferred"
                    );
                }
            });
        }
        Ok(())
    }

    async fn preflight(
        &self,
        input: &LaunchPreflightInput,
    ) -> Result<LaunchPreflight, AcpWorkbenchError> {
        validate_selected_workspace(&input.workspace).await?;
        self.runtime_catalog.prepare(&input.agent_id).await?;
        self.orchestration.preflight(input).await
    }

    async fn check_client_readiness(&self, agent_id: String) -> AcpClientReadiness {
        if let Err(error) = require_nonblank("agentId", &agent_id) {
            return AcpClientReadiness::unavailable(agent_id, error);
        }
        if let Err(error) = self.runtime_catalog.prepare(&agent_id).await {
            return AcpClientReadiness::unavailable(agent_id, error);
        }
        match self.orchestration.connect(&agent_id).await {
            Ok(_) => AcpClientReadiness {
                agent_id,
                status: AcpClientReadinessStatus::Ready,
                reason_code: None,
                reason: None,
                retryable: false,
            },
            Err(error) => AcpClientReadiness::unavailable(agent_id, error),
        }
    }

    async fn read_feedback_detail(
        &self,
        request_id: String,
    ) -> Result<FeedbackDetailView, AcpWorkbenchError> {
        let detail = self.load_feedback_detail(&request_id).await?;
        let published_feedback = self
            .read_published_feedback(detail.delivery.as_ref())
            .await?;
        Ok(FeedbackDetailView {
            request: detail.request,
            session: detail.session,
            delivery: detail.delivery,
            draft: detail.draft.map(Into::into),
            published_feedback,
        })
    }

    async fn read_published_feedback(
        &self,
        delivery: Option<&FeedbackDeliveryRecord>,
    ) -> Result<Option<PublishedFeedbackView>, AcpWorkbenchError> {
        let Some(package) = delivery.and_then(|delivery| delivery.package.as_ref()) else {
            return Ok(None);
        };
        let feedback = package
            .artifacts
            .iter()
            .find(|artifact| artifact.role == ArtifactRole::Feedback)
            .ok_or_else(|| {
                AcpWorkbenchError::new(
                    "CORRUPT_DATA",
                    "the persisted Feedback Package omitted feedback.md",
                    false,
                )
            })?;
        let uncooked = package
            .artifacts
            .iter()
            .find(|artifact| artifact.role == ArtifactRole::Uncooked);
        Ok(Some(PublishedFeedbackView {
            markdown: self.read_package_text(feedback).await?,
            uncooked_markdown: match uncooked {
                Some(artifact) => Some(self.read_package_text(artifact).await?),
                None => None,
            },
        }))
    }

    async fn read_package_text(
        &self,
        artifact: &PackageArtifact,
    ) -> Result<String, AcpWorkbenchError> {
        let bytes = self
            .artifacts
            .open_verified(&artifact.storage_key, &artifact.sha256)
            .await
            .map_err(rambledesk_core::kernel::CoreError::from)?;
        String::from_utf8(bytes).map_err(|_| {
            AcpWorkbenchError::new(
                "CORRUPT_DATA",
                format!("{} is not valid UTF-8", artifact.display_name),
                false,
            )
        })
    }

    async fn load_feedback_detail(
        &self,
        request_id: &str,
    ) -> Result<V3FeedbackDetail, AcpWorkbenchError> {
        self.facts
            .read_feedback_detail(RequestId::new(request_id))
            .await
            .map_err(rambledesk_core::kernel::CoreError::from)
            .map_err(Into::into)
    }

    /// Finds a Managed ACP Feedback Request for shared Desktop capabilities
    /// such as voice Ramble. The source-aware caller treats `None` as not found
    /// and never consults the Adapter Runtime for this request.
    pub(super) async fn voice_feedback_status(
        &self,
        request_id: &str,
    ) -> Result<Option<FeedbackRequestStatus>, AcpWorkbenchError> {
        match self.load_feedback_detail(request_id).await {
            Ok(detail) => Ok(Some(detail.request.status)),
            Err(error) if error.code == "REQUEST_NOT_FOUND" => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn read_ramble_draft_detail(
        &self,
        draft_id: String,
    ) -> Result<Option<DraftSnapshot>, AcpWorkbenchError> {
        self.facts
            .read_ramble_draft_detail(DraftId::new(draft_id))
            .await
            .map_err(rambledesk_core::kernel::CoreError::from)
            .map_err(Into::into)
    }

    async fn launch(
        &self,
        input: LaunchDraftInput,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        let preflight_input = LaunchPreflightInput {
            workspace: input.workspace.clone(),
            agent_id: input.agent_id.clone(),
        };
        let preflight = self.preflight(&preflight_input).await?;
        validate_launch_selection(&input, &preflight)?;
        let launch_profile_id = self
            .runtime_catalog
            .launch_profile_id(&input.agent_id)
            .ok_or_else(|| {
                AcpWorkbenchError::new(
                    "LAUNCH_PROFILE_NOT_FOUND",
                    format!("no ACP Launch Profile is configured for {}", input.agent_id),
                    false,
                )
            })?;
        let launch = LaunchSubmission {
            submission_id: SubmissionId::new(input.submission_id),
            submission_digest_assertion: None,
            title: title_from_markdown(&input.body_markdown),
            launch_configuration: LaunchConfiguration {
                agent_profile_id: input.agent_id.clone(),
                launch_profile_id,
                workspace_reference: input.workspace,
                model: None,
                reasoning_effort: None,
                // Retained only as a legacy Core projection. The ACP Client
                // reads the versioned, ordered generic config below.
                access_mode: AccessMode::WorkspaceWrite,
                agent_config_json: serde_json::to_string(
                    &rambledesk_acp_client::AgentLaunchConfig {
                        version: 1,
                        schema_digest: input.schema_digest,
                        values: input.config_values,
                    },
                )
                .map_err(|error| {
                    AcpWorkbenchError::new(
                        "ACP_CONFIG_SERIALIZATION_FAILED",
                        format!("could not serialize Agent Launch Config: {error}"),
                        false,
                    )
                })?,
            },
            ramble: RambleContent {
                document_json: input.document_json,
                body_markdown: input.body_markdown,
                artifacts: Vec::new(),
            },
        };
        let outcome = self.core.launch(launch).await?;
        self.orchestration
            .reconcile(outcome.session_id)
            .await
            .map_err(AcpWorkbenchError::after_durable_launch)?;
        self.read().await
    }

    async fn save_draft(
        &self,
        input: DraftInput,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        let durable = self
            .core
            .read_workbench(WorkbenchQuery { session_id: None })
            .await?;
        let request = durable
            .waiting_feedback
            .iter()
            .find(|request| request.request_id.as_str() == input.request_id)
            .ok_or_else(|| {
                AcpWorkbenchError::new(
                    "REQUEST_NOT_FOUND",
                    "the waiting Feedback Request was not found",
                    false,
                )
            })?;
        let draft_id = durable
            .drafts
            .iter()
            .find(|draft| draft.request_id.as_ref() == Some(&request.request_id))
            .map(|draft| draft.draft_id.clone())
            .unwrap_or_else(|| rambledesk_core::kernel::DraftId::new(input.request_id.clone()));
        self.core
            .mutate_draft(DraftMutation::Save(SaveDraft {
                draft_id,
                intent: RambleIntent::Feedback,
                session_id: Some(request.session_id.clone()),
                request_id: Some(request.request_id.clone()),
                launch_configuration: None,
                document_json: input.document_json,
                body_markdown: input.body_markdown,
                expected_revision: input.expected_revision,
            }))
            .await?;
        self.read().await
    }

    async fn submit_feedback(
        &self,
        input: FeedbackDecisionInput,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        let durable = self
            .core
            .read_workbench(WorkbenchQuery { session_id: None })
            .await?;
        let request = durable
            .waiting_feedback
            .iter()
            .find(|request| request.request_id.as_str() == input.request_id)
            .ok_or_else(|| {
                AcpWorkbenchError::new(
                    "REQUEST_NOT_FOUND",
                    "the waiting Feedback Request was not found",
                    false,
                )
            })?;
        let artifacts = match durable
            .drafts
            .iter()
            .find(|draft| draft.request_id.as_ref() == Some(&request.request_id))
        {
            Some(draft) => self.read_draft_artifacts(draft).await?,
            None => Vec::new(),
        };
        let outcome = self
            .core
            .resolve_feedback(ResolveFeedbackRequest::Submit(FeedbackSubmission {
                submission_id: SubmissionId::new(input.submission_id),
                request_id: request.request_id.clone(),
                expected_draft_revision: input.expected_revision,
                submission_digest_assertion: None,
                document_json: input.document_json,
                uncooked_markdown: input
                    .uncooked_markdown
                    .unwrap_or_else(|| input.body_markdown.clone()),
                feedback_markdown: input.cooked_markdown.unwrap_or(input.body_markdown),
                cooking_model: input.cooking_model,
                artifacts,
            }))
            .await?;
        self.reconcile_committed(outcome.request.session_id.clone(), "Feedback Submission")
            .await;
        self.read().await
    }

    async fn cancel_feedback(
        &self,
        request_id: String,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        let outcome = self
            .core
            .resolve_feedback(ResolveFeedbackRequest::Cancel(CancelFeedbackRequest {
                request_id: RequestId::new(request_id),
                reason: "Cancelled by the human in RambleDesk.".to_owned(),
            }))
            .await?;
        self.reconcile_committed(outcome.request.session_id.clone(), "Feedback Cancellation")
            .await;
        self.read().await
    }

    async fn read_draft_artifacts(
        &self,
        draft: &rambledesk_core::kernel::DraftSnapshot,
    ) -> Result<Vec<ArtifactInput>, AcpWorkbenchError> {
        let mut inputs = Vec::with_capacity(draft.artifacts.len());
        for artifact in &draft.artifacts {
            let contents = self
                .artifacts
                .open_verified(&artifact.storage_key, &artifact.sha256)
                .await
                .map_err(rambledesk_core::kernel::CoreError::from)?;
            inputs.push(ArtifactInput {
                display_name: artifact.display_name.clone(),
                media_type: artifact.media_type.clone(),
                contents,
            });
        }
        Ok(inputs)
    }

    async fn reconcile_committed(
        &self,
        session_id: rambledesk_core::kernel::SessionId,
        fact: &str,
    ) {
        if let Err(error) = self.orchestration.reconcile(session_id.clone()).await {
            tracing::warn!(
                code = %error.code,
                %session_id,
                fact,
                "ACP reconcile deferred after durable local fact"
            );
        }
    }

    async fn answer_permission(
        &self,
        input: PermissionAnswerInput,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        require_nonblank("requestId", &input.request_id)?;
        require_nonblank("optionId", &input.option_id)?;
        self.orchestration.answer_permission(input).await?;
        self.read().await
    }

    async fn answer_question(
        &self,
        input: QuestionAnswerInput,
    ) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
        require_nonblank("requestId", &input.request_id)?;
        if input.skipped && !input.choice_ids.is_empty() {
            return Err(AcpWorkbenchError::new(
                "INVALID_ARGUMENT",
                "a skipped Ask Question must not include choices",
                false,
            ));
        }
        if !input.skipped && input.choice_ids.is_empty() {
            return Err(AcpWorkbenchError::new(
                "INVALID_ARGUMENT",
                "an Ask Question answer must include at least one choice",
                false,
            ));
        }
        let mut unique = HashSet::new();
        if input
            .choice_ids
            .iter()
            .any(|choice| choice.trim().is_empty() || !unique.insert(choice.as_str()))
        {
            return Err(AcpWorkbenchError::new(
                "INVALID_ARGUMENT",
                "Ask Question choice ids must be nonblank and unique",
                false,
            ));
        }
        self.orchestration.answer_question(input).await?;
        self.read().await
    }

    pub(super) async fn shutdown(&self) {
        if let Err(error) = self.orchestration.shutdown().await {
            tracing::warn!(code = %error.code, "ACP Client shutdown failed");
        }
    }
}

#[tauri::command]
pub(super) async fn read_acp_workbench(
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.read().await
}

#[tauri::command]
pub(super) async fn read_feedback_v3(
    request_id: String,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<FeedbackDetailView, AcpWorkbenchError> {
    state.read_feedback_detail(request_id).await
}

#[tauri::command]
pub(super) async fn read_ramble_draft_v3(
    draft_id: String,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<Option<DraftSnapshot>, AcpWorkbenchError> {
    state.read_ramble_draft_detail(draft_id).await
}

#[tauri::command]
pub(super) async fn preflight_acp_launch(
    input: LaunchPreflightInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<LaunchPreflight, AcpWorkbenchError> {
    state.preflight(&input).await
}

#[tauri::command]
pub(super) async fn connect_acp_client(
    agent_id: String,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpClientReadiness, AcpWorkbenchError> {
    Ok(state.check_client_readiness(agent_id).await)
}

#[tauri::command]
pub(super) async fn launch_ramble_v3(
    input: LaunchDraftInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.launch(input).await
}

#[tauri::command]
pub(super) async fn save_ramble_draft_v3(
    input: DraftInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.save_draft(input).await
}

#[tauri::command]
pub(super) async fn add_feedback_draft_artifact_v3(
    input: AddDraftArtifactInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<DraftSnapshotView, AcpWorkbenchError> {
    state.add_draft_artifact(input).await
}

#[tauri::command]
pub(super) async fn remove_feedback_draft_artifact_v3(
    input: RemoveDraftArtifactInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<DraftSnapshotView, AcpWorkbenchError> {
    state.remove_draft_artifact(input).await
}

#[tauri::command]
pub(super) async fn reorder_feedback_draft_artifacts_v3(
    input: ReorderDraftArtifactsInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<DraftSnapshotView, AcpWorkbenchError> {
    state.reorder_draft_artifacts(input).await
}

#[tauri::command]
pub(super) async fn read_feedback_draft_artifact_v3(
    request_id: String,
    artifact_id: String,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<tauri::ipc::Response, AcpWorkbenchError> {
    state
        .read_draft_artifact(request_id, artifact_id)
        .await
        .map(tauri::ipc::Response::new)
}

#[tauri::command]
pub(super) async fn submit_feedback_v3(
    input: FeedbackDecisionInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.submit_feedback(input).await
}

#[tauri::command]
pub(super) async fn cancel_feedback_v3(
    request_id: String,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.cancel_feedback(request_id).await
}

#[tauri::command]
pub(super) async fn answer_acp_permission(
    input: PermissionAnswerInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.answer_permission(input).await
}

#[tauri::command]
pub(super) async fn answer_acp_question(
    input: QuestionAnswerInput,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<AcpWorkbenchSnapshot, AcpWorkbenchError> {
    state.answer_question(input).await
}

#[cfg(test)]
mod tests;
