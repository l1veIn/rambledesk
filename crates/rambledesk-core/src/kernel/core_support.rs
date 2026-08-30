use std::collections::HashSet;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use super::{
    AcpSessionLinkId, AgentWorkId, AgentWorkKind, AgentWorkPayload, AgentWorkRecord,
    AgentWorkState, ArtifactId, ArtifactInput, ArtifactRole, CoreError, CoreErrorCode, DeliveryId,
    DraftId, FeedbackAction, LaunchConfiguration, PackageArtifact, PackageId, RambleIntent,
    RequestId, SaveDraft, SessionId, StoredBlob, SubmissionArtifact, WorkClaimToken,
    digest::agent_work_payload_digest,
};

pub(super) const PACKAGE_SCHEMA_VERSION: u32 = 3;
const MAX_ARTIFACTS: usize = 20;
const MAX_ACTIONS: usize = 20;

pub(super) trait GeneratedId {
    fn new_id() -> Self;
}

macro_rules! impl_generated_id {
    ($name:ident) => {
        impl GeneratedId for $name {
            fn new_id() -> Self {
                Self::new(Uuid::now_v7().to_string())
            }
        }
    };
}

impl_generated_id!(SessionId);
impl_generated_id!(RequestId);
impl_generated_id!(PackageId);
impl_generated_id!(ArtifactId);
impl_generated_id!(DeliveryId);
impl_generated_id!(AgentWorkId);
impl_generated_id!(AcpSessionLinkId);
impl_generated_id!(WorkClaimToken);
impl_generated_id!(DraftId);

pub(super) fn feedback_resume_work(
    work_id: AgentWorkId,
    session_id: SessionId,
    delivery_id: DeliveryId,
    request_id: RequestId,
    created_at: String,
) -> AgentWorkRecord {
    AgentWorkRecord {
        work_id,
        session_id,
        kind: AgentWorkKind::FeedbackResume,
        source_id: delivery_id.to_string(),
        payload_digest: agent_work_payload_digest(
            "feedback_resume",
            delivery_id.as_str(),
            request_id.as_str(),
        ),
        payload: AgentWorkPayload::FeedbackResume {
            delivery_id,
            request_id,
        },
        state: AgentWorkState::Pending,
        attempt_count: 0,
        last_error_code: None,
        last_error_at: None,
        created_at,
        completed_at: None,
    }
}

pub(super) fn package_artifact(
    artifact_id: ArtifactId,
    role: ArtifactRole,
    position: u32,
    input: &ArtifactInput,
    blob: &StoredBlob,
) -> PackageArtifact {
    PackageArtifact {
        artifact_id,
        role,
        position,
        display_name: input.display_name.clone(),
        media_type: input.media_type.clone(),
        size_bytes: blob.size_bytes,
        sha256: blob.sha256.clone(),
        storage_key: blob.storage_key.clone(),
    }
}

pub(super) fn submission_artifact(
    artifact_id: ArtifactId,
    position: u32,
    input: &ArtifactInput,
    blob: StoredBlob,
) -> SubmissionArtifact {
    SubmissionArtifact {
        artifact_id,
        position,
        display_name: input.display_name.clone(),
        media_type: input.media_type.clone(),
        size_bytes: blob.size_bytes,
        sha256: blob.sha256,
        storage_key: blob.storage_key,
    }
}

pub(super) fn validate_launch_configuration(
    configuration: &LaunchConfiguration,
) -> Result<(), CoreError> {
    validate_nonblank("agent_profile_id", &configuration.agent_profile_id, 1, 128)?;
    validate_nonblank(
        "launch_profile_id",
        &configuration.launch_profile_id,
        1,
        128,
    )?;
    validate_text(
        "workspace_reference",
        &configuration.workspace_reference,
        1,
        4_096,
    )?;
    validate_text(
        "agent_config_json",
        &configuration.agent_config_json,
        1,
        100_000,
    )?;
    if let Some(value) = &configuration.model {
        validate_text("model", value, 1, 500)?;
    }
    if let Some(value) = &configuration.reasoning_effort {
        validate_text("reasoning_effort", value, 1, 500)?;
    }
    Ok(())
}

pub(super) fn validate_ramble(document_json: &str, body_markdown: &str) -> Result<(), CoreError> {
    validate_text("document_json", document_json, 1, 1_000_000)?;
    validate_text("body_markdown", body_markdown, 1, 100_000)
}

pub(super) fn validate_draft_identity(input: &SaveDraft) -> Result<(), CoreError> {
    let valid = match input.intent {
        RambleIntent::Launch => {
            input.session_id.is_none()
                && input.request_id.is_none()
                && input.launch_configuration.is_some()
        }
        RambleIntent::Steering => {
            input.session_id.is_some()
                && input.request_id.is_none()
                && input.launch_configuration.is_none()
        }
        RambleIntent::Feedback => {
            input.session_id.is_some()
                && input.request_id.is_some()
                && input.launch_configuration.is_none()
        }
    };
    if !valid {
        return Err(CoreError::invalid_argument(
            "draft identity does not match its Ramble intent",
        ));
    }
    if let Some(configuration) = &input.launch_configuration {
        validate_launch_configuration(configuration)?;
    }
    Ok(())
}

pub(super) fn validate_actions(actions: &[FeedbackAction]) -> Result<(), CoreError> {
    if !(1..=MAX_ACTIONS).contains(&actions.len()) {
        return Err(CoreError::invalid_argument(
            "actions must contain between 1 and 20 items",
        ));
    }
    let mut ids = HashSet::new();
    for action in actions {
        if !valid_action_id(&action.id) || !ids.insert(action.id.as_str()) {
            return Err(CoreError::invalid_argument(
                "action ids must be unique and match ^[a-z0-9][a-z0-9_-]{0,63}$",
            ));
        }
        validate_nonblank("action.instruction", &action.instruction, 1, 2_000)?;
    }
    Ok(())
}

pub(super) fn validate_context_refs(refs: &[super::ContextReference]) -> Result<(), CoreError> {
    if refs.len() > 20 {
        return Err(CoreError::invalid_argument(
            "context_refs cannot contain more than 20 items",
        ));
    }
    for value in refs {
        validate_nonblank("context_ref.label", &value.label, 1, 256)?;
        validate_nonblank("context_ref.uri", &value.uri, 1, 4_096)?;
    }
    Ok(())
}

pub(super) fn validate_artifacts(artifacts: &[ArtifactInput]) -> Result<(), CoreError> {
    if artifacts.len() > MAX_ARTIFACTS {
        return Err(CoreError::invalid_argument(
            "artifacts cannot contain more than 20 items",
        ));
    }
    for artifact in artifacts {
        validate_nonblank("artifact.display_name", &artifact.display_name, 1, 255)?;
        if artifact.display_name.contains(['/', '\\']) {
            return Err(CoreError::invalid_argument(
                "artifact display_name must not contain path separators",
            ));
        }
        validate_nonblank("artifact.media_type", &artifact.media_type, 1, 255)?;
        if artifact.contents.is_empty() {
            return Err(CoreError::invalid_argument(
                "artifact contents cannot be empty",
            ));
        }
    }
    Ok(())
}

pub(super) fn verify_submission_digest(expected: &str, actual: &str) -> Result<(), CoreError> {
    validate_digest("submission_digest_assertion", expected)?;
    if expected == actual {
        Ok(())
    } else {
        Err(CoreError::invalid_argument(
            "submission_digest_assertion does not match the canonical submission content",
        ))
    }
}

pub(super) fn validate_digest(field: &str, value: &str) -> Result<(), CoreError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(CoreError::invalid_argument(format!(
            "{field} must use sha256:<lowercase hex>",
        )));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err(CoreError::invalid_argument(format!(
            "{field} must use sha256:<lowercase hex>",
        )));
    }
    Ok(())
}

pub(super) fn validate_id(field: &str, value: &str) -> Result<(), CoreError> {
    validate_text(field, value, 1, 256)
}

pub(super) fn validate_text(
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), CoreError> {
    let count = value.chars().count();
    if !(min..=max).contains(&count) || value.contains('\0') {
        return Err(CoreError::invalid_argument(format!(
            "{field} must contain between {min} and {max} valid characters",
        )));
    }
    Ok(())
}

pub(super) fn validate_nonblank(
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), CoreError> {
    validate_text(field, value, min, max)?;
    if value.trim().is_empty() {
        return Err(CoreError::invalid_argument(format!(
            "{field} must not be blank",
        )));
    }
    Ok(())
}

fn valid_action_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|value| {
            value.is_ascii_lowercase() || value.is_ascii_digit() || matches!(value, b'_' | b'-')
        })
}

pub(super) fn now() -> Result<String, CoreError> {
    format_time(OffsetDateTime::now_utc())
}

pub(super) fn format_time(value: OffsetDateTime) -> Result<String, CoreError> {
    value.format(&Rfc3339).map_err(|_| {
        CoreError::new(
            CoreErrorCode::CorruptData,
            "UTC timestamp could not be formatted",
            false,
        )
    })
}

pub(super) fn unexpected_store_outcome() -> CoreError {
    CoreError::new(
        CoreErrorCode::CorruptData,
        "Fact Store returned the wrong outcome variant",
        false,
    )
}
