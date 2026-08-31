use rambledesk_core::kernel::{
    AddDraftArtifact, ArtifactId, ArtifactInput, DraftMutation, DraftSnapshot,
    FeedbackRequestStatus, RemoveDraftArtifact, ReorderDraftArtifacts, RequestId,
    ports::ArtifactStore,
};
use std::path::{Path, PathBuf};

use crate::{clipboard_capture::ClipboardCaptureState, screen_capture::ScreenCaptureState};

use super::{
    AcpWorkbenchError, AcpWorkbenchState, AddDraftArtifactInput, DraftSnapshotView,
    RemoveDraftArtifactInput, ReorderDraftArtifactsInput,
};

#[tauri::command]
pub(crate) async fn import_feedback_draft_artifact_path_v3(
    request_id: String,
    path: PathBuf,
    expected_revision: u64,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<DraftSnapshotView, AcpWorkbenchError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AcpWorkbenchError::new(
                "INVALID_ARTIFACT_PATH",
                "the Artifact path has no UTF-8 file name",
                false,
            )
        })?
        .to_owned();
    let contents = tokio::fs::read(&path).await.map_err(|_| {
        AcpWorkbenchError::new(
            "INVALID_ARTIFACT_PATH",
            "the Artifact path could not be read",
            false,
        )
    })?;
    state
        .add_draft_artifact(AddDraftArtifactInput {
            request_id,
            expected_revision,
            media_type: media_type_for_path(&path).to_owned(),
            file_name,
            contents,
        })
        .await
}

#[tauri::command]
pub(crate) async fn add_completed_screen_capture_v3(
    request_id: String,
    capture_session_id: String,
    expected_revision: u64,
    capture_state: tauri::State<'_, ScreenCaptureState>,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<DraftSnapshotView, AcpWorkbenchError> {
    let contents = capture_state
        .take_completed_png(&capture_session_id)
        .map_err(|message| AcpWorkbenchError::new("CAPTURE_NOT_FOUND", message, false))?;
    state
        .add_draft_artifact(AddDraftArtifactInput {
            request_id,
            expected_revision,
            file_name: format!("ramble-screenshot-{capture_session_id}.png"),
            media_type: "image/png".to_owned(),
            contents,
        })
        .await
}

#[tauri::command]
pub(crate) async fn add_completed_clipboard_capture_v3(
    request_id: String,
    capture_id: String,
    ramble_context_id: String,
    file_name: String,
    expected_revision: u64,
    clipboard: tauri::State<'_, ClipboardCaptureState>,
    state: tauri::State<'_, AcpWorkbenchState>,
) -> Result<DraftSnapshotView, AcpWorkbenchError> {
    let contents = clipboard
        .take_image(&capture_id, &request_id, &ramble_context_id)
        .map_err(|message| AcpWorkbenchError::new("CLIPBOARD_CAPTURE_NOT_FOUND", message, false))?;
    let file_name = if file_name.starts_with("ramble-clipboard-") && file_name.ends_with(".png") {
        file_name
    } else {
        format!("ramble-clipboard-{capture_id}.png")
    };
    state
        .add_draft_artifact(AddDraftArtifactInput {
            request_id,
            expected_revision,
            file_name,
            media_type: "image/png".to_owned(),
            contents,
        })
        .await
}

fn media_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" => "text/plain",
        "json" => "application/json",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

impl AcpWorkbenchState {
    pub(super) async fn add_draft_artifact(
        &self,
        input: AddDraftArtifactInput,
    ) -> Result<DraftSnapshotView, AcpWorkbenchError> {
        let draft = self.editable_feedback_draft(&input.request_id).await?;
        require_revision(&draft, input.expected_revision)?;
        self.core
            .mutate_draft(DraftMutation::AddArtifact(AddDraftArtifact {
                draft_id: draft.draft_id,
                expected_revision: input.expected_revision,
                artifact: ArtifactInput {
                    display_name: input.file_name,
                    media_type: input.media_type,
                    contents: input.contents,
                },
            }))
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub(super) async fn remove_draft_artifact(
        &self,
        input: RemoveDraftArtifactInput,
    ) -> Result<DraftSnapshotView, AcpWorkbenchError> {
        let draft = self.editable_feedback_draft(&input.request_id).await?;
        require_revision(&draft, input.expected_revision)?;
        let artifact_id = ArtifactId::new(input.artifact_id);
        require_owned_artifact(&draft, &artifact_id)?;
        self.core
            .mutate_draft(DraftMutation::RemoveArtifact(RemoveDraftArtifact {
                draft_id: draft.draft_id,
                expected_revision: input.expected_revision,
                artifact_id,
            }))
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub(super) async fn reorder_draft_artifacts(
        &self,
        input: ReorderDraftArtifactsInput,
    ) -> Result<DraftSnapshotView, AcpWorkbenchError> {
        let draft = self.editable_feedback_draft(&input.request_id).await?;
        require_revision(&draft, input.expected_revision)?;
        let artifact_ids = input
            .artifact_ids
            .into_iter()
            .map(ArtifactId::new)
            .collect::<Vec<_>>();
        for artifact_id in &artifact_ids {
            require_owned_artifact(&draft, artifact_id)?;
        }
        self.core
            .mutate_draft(DraftMutation::ReorderArtifacts(ReorderDraftArtifacts {
                draft_id: draft.draft_id,
                expected_revision: input.expected_revision,
                artifact_ids,
            }))
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub(super) async fn read_draft_artifact(
        &self,
        request_id: String,
        artifact_id: String,
    ) -> Result<Vec<u8>, AcpWorkbenchError> {
        let draft = self.editable_feedback_draft(&request_id).await?;
        let artifact_id = ArtifactId::new(artifact_id);
        let artifact = require_owned_artifact(&draft, &artifact_id)?;
        self.artifacts
            .open_verified(&artifact.storage_key, &artifact.sha256)
            .await
            .map_err(rambledesk_core::kernel::CoreError::from)
            .map_err(Into::into)
    }

    async fn editable_feedback_draft(
        &self,
        request_id: &str,
    ) -> Result<DraftSnapshot, AcpWorkbenchError> {
        let detail = self.load_feedback_detail(request_id).await?;
        if detail.request.status != FeedbackRequestStatus::Waiting {
            return Err(AcpWorkbenchError::new(
                "REQUEST_TERMINAL",
                "the Feedback Request is already terminal",
                false,
            ));
        }
        let draft = detail.draft.ok_or_else(|| {
            AcpWorkbenchError::new(
                "DRAFT_NOT_FOUND",
                "save the Feedback Draft before changing its Artifacts",
                false,
            )
        })?;
        if draft.request_id.as_ref() != Some(&RequestId::new(request_id)) {
            return Err(AcpWorkbenchError::new(
                "DRAFT_CONFLICT",
                "the Draft does not belong to this Feedback Request",
                false,
            ));
        }
        Ok(draft)
    }
}

fn require_revision(
    draft: &DraftSnapshot,
    expected_revision: u64,
) -> Result<(), AcpWorkbenchError> {
    if draft.revision == expected_revision {
        Ok(())
    } else {
        Err(AcpWorkbenchError::new(
            "DRAFT_CONFLICT",
            "the draft revision changed",
            false,
        ))
    }
}

fn require_owned_artifact<'a>(
    draft: &'a DraftSnapshot,
    artifact_id: &ArtifactId,
) -> Result<&'a rambledesk_core::kernel::DraftArtifact, AcpWorkbenchError> {
    draft
        .artifacts
        .iter()
        .find(|artifact| &artifact.artifact_id == artifact_id)
        .ok_or_else(|| {
            AcpWorkbenchError::new(
                "ARTIFACT_NOT_FOUND",
                "the Draft Artifact was not found for this Feedback Request",
                false,
            )
        })
}
