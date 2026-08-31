use rambledesk_acp_client::{LaunchConfigOption, LaunchConfigSelection};
use rambledesk_core::kernel::{
    AccessMode, DraftSnapshot, FeedbackDeliveryRecord, FeedbackRequestSnapshot, RambleIntent,
    SessionRecord,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchDraftInput {
    pub submission_id: String,
    pub workspace: String,
    pub agent_id: String,
    pub schema_digest: String,
    pub config_values: Vec<LaunchConfigSelection>,
    pub document_json: String,
    pub body_markdown: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchPreflightInput {
    pub workspace: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DraftInput {
    pub request_id: String,
    pub expected_revision: u64,
    pub document_json: String,
    pub body_markdown: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddDraftArtifactInput {
    pub request_id: String,
    pub expected_revision: u64,
    pub file_name: String,
    pub media_type: String,
    pub contents: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoveDraftArtifactInput {
    pub request_id: String,
    pub artifact_id: String,
    pub expected_revision: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReorderDraftArtifactsInput {
    pub request_id: String,
    pub artifact_ids: Vec<String>,
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeedbackDetailView {
    pub request: FeedbackRequestSnapshot,
    pub session: SessionRecord,
    pub delivery: Option<FeedbackDeliveryRecord>,
    pub draft: Option<DraftSnapshotView>,
    pub published_feedback: Option<PublishedFeedbackView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PublishedFeedbackView {
    pub markdown: String,
    pub uncooked_markdown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DraftSnapshotView {
    pub draft_id: String,
    pub intent: RambleIntent,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub document_json: String,
    pub body_markdown: String,
    pub revision: u64,
    pub artifacts: Vec<DraftArtifactView>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<DraftSnapshot> for DraftSnapshotView {
    fn from(draft: DraftSnapshot) -> Self {
        Self {
            draft_id: draft.draft_id.to_string(),
            intent: draft.intent,
            session_id: draft.session_id.map(|id| id.to_string()),
            request_id: draft.request_id.map(|id| id.to_string()),
            document_json: draft.document_json,
            body_markdown: draft.body_markdown,
            revision: draft.revision,
            artifacts: draft
                .artifacts
                .into_iter()
                .map(|artifact| DraftArtifactView {
                    artifact_id: artifact.artifact_id.to_string(),
                    file_name: artifact.display_name,
                    media_type: artifact.media_type,
                    byte_size: artifact.size_bytes,
                    sha256: artifact.sha256,
                    position: artifact.position,
                })
                .collect(),
            created_at: draft.created_at,
            updated_at: draft.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DraftArtifactView {
    pub artifact_id: String,
    pub file_name: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub position: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeedbackDecisionInput {
    pub submission_id: String,
    pub request_id: String,
    pub expected_revision: u64,
    pub document_json: String,
    pub body_markdown: String,
    pub cooked_markdown: Option<String>,
    pub cooking_model: Option<String>,
    pub uncooked_markdown: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PermissionAnswerInput {
    pub request_id: String,
    pub option_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionAnswerInput {
    pub request_id: String,
    pub choice_ids: Vec<String>,
    pub skipped: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenameAcpSessionInput {
    pub session_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetAcpSessionPinnedInput {
    pub session_id: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentSummary {
    pub id: String,
    pub label: String,
    pub icon_svg: String,
    pub supports_structured_ramble: bool,
    pub models: Vec<String>,
    pub reasoning_efforts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionStatus {
    Running,
    Waiting,
    Offline,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcpSessionSummary {
    pub session_id: String,
    pub title: String,
    pub agent_id: String,
    pub agent_label: String,
    pub workspace: String,
    pub model: String,
    pub reasoning_effort: String,
    pub access_mode: AccessMode,
    pub status: SessionStatus,
    pub pending_count: u32,
    pub pinned_at: Option<String>,
    pub archived_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AttentionStatus {
    Waiting,
    Submitted,
    Cancelled,
}

// The concrete ACP Client Adapter constructs these live-only variants after
// it is merged into this Desktop seam.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PermissionTone {
    Allow,
    Deny,
    Neutral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PermissionOption {
    pub id: String,
    pub label: String,
    pub tone: PermissionTone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QuestionChoice {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(dead_code)]
pub(super) enum AttentionItem {
    Feedback {
        id: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        title: String,
        #[serde(rename = "createdAt")]
        created_at: String,
        #[serde(rename = "updatedAt")]
        updated_at: String,
        status: AttentionStatus,
        summary: String,
        instructions: String,
        actions: Vec<String>,
        #[serde(rename = "draftDocument")]
        draft_document: Option<serde_json::Value>,
        #[serde(rename = "draftMarkdown")]
        draft_markdown: String,
        #[serde(rename = "draftRevision")]
        draft_revision: u64,
    },
    Permission {
        id: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        title: String,
        #[serde(rename = "createdAt")]
        created_at: String,
        status: AttentionStatus,
        description: String,
        #[serde(rename = "toolCall")]
        tool_call: serde_json::Value,
        #[serde(rename = "toolTitle")]
        tool_title: String,
        command: Option<String>,
        path: Option<String>,
        options: Vec<PermissionOption>,
    },
    Question {
        id: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        title: String,
        #[serde(rename = "createdAt")]
        created_at: String,
        status: AttentionStatus,
        prompt: String,
        choices: Vec<QuestionChoice>,
        multiple: bool,
        #[serde(rename = "allowSkip")]
        allow_skip: bool,
        #[serde(rename = "unsupportedReason", skip_serializing_if = "Option::is_none")]
        unsupported_reason: Option<String>,
    },
}

impl AttentionItem {
    pub(super) fn session_id(&self) -> &str {
        match self {
            Self::Feedback { session_id, .. }
            | Self::Permission { session_id, .. }
            | Self::Question { session_id, .. } => session_id,
        }
    }

    pub(super) fn is_waiting(&self) -> bool {
        matches!(
            self,
            Self::Feedback {
                status: AttentionStatus::Waiting,
                ..
            } | Self::Permission {
                status: AttentionStatus::Waiting,
                ..
            } | Self::Question {
                status: AttentionStatus::Waiting,
                ..
            }
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcpWorkbenchSnapshot {
    pub(super) sessions: Vec<AcpSessionSummary>,
    pub(super) attention_items: Vec<AttentionItem>,
    pub(super) agents: Vec<AgentSummary>,
    /// Ephemeral projection of currently observed ACP runs. This is deliberately
    /// absent from SQLite and is cleared when the Desktop process shuts down.
    pub(super) timelines: Vec<SessionTimeline>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionTimeline {
    pub session_id: String,
    pub live_only: bool,
    pub turns: Vec<TimelineTurn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TimelineTurnStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TimelineTurn {
    pub turn_id: String,
    pub status: TimelineTurnStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub entries: Vec<TimelineEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TimelineEntryKind {
    Thought,
    Tool,
    Message,
    Status,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TimelineEntryStatus {
    Running,
    Completed,
    Failed,
    Waiting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TimelineEntry {
    pub id: String,
    pub kind: TimelineEntryKind,
    pub title: String,
    pub content: String,
    pub status: TimelineEntryStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchPreflight {
    pub agent_id: String,
    pub schema_digest: String,
    pub config_options: Vec<LaunchConfigOption>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AcpClientReadinessStatus {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcpClientReadiness {
    pub agent_id: String,
    pub status: AcpClientReadinessStatus,
    pub reason_code: Option<String>,
    pub reason: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcpWorkbenchError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub local_fact_committed: bool,
}

impl AcpWorkbenchError {
    pub(super) fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
            local_fact_committed: false,
        }
    }

    pub(super) fn after_durable_launch(mut self) -> Self {
        self.message = format!(
            "RambleDesk saved the Launch as an Offline Session, but the Agent did not start: {}",
            self.message
        );
        self.local_fact_committed = true;
        self
    }
}

impl AcpClientReadiness {
    pub(super) fn unavailable(agent_id: String, error: AcpWorkbenchError) -> Self {
        let reason = match error.code.as_str() {
            "ACP_RUNTIME_MISSING" => "Node.js with npx is not installed",
            "ACP_AGENT_LAUNCH_FAILED" => "the ACP Server did not start",
            "ACP_OPERATION_TIMED_OUT" => "preparing the ACP Server timed out",
            "ACP_PROTOCOL_VIOLATION" | "ACP_RPC_ERROR" => "the ACP protocol handshake failed",
            "ACP_AGENT_UNAVAILABLE" => "the ACP Agent reported that it is unavailable",
            "ACP_AUTHENTICATION_REQUIRED" => {
                "the Agent is installed but needs a signed-in and licensed account"
            }
            "ACP_SESSION_TOOLSET_UNSUPPORTED" => {
                "the Agent can connect over ACP but cannot receive RambleDesk Feedback tools"
            }
            "ACP_PLATFORM_UNSUPPORTED" => "this Agent is not available for this platform",
            "ACP_INSTALL_FAILED" => "RambleDesk could not install the ACP Agent client",
            "ACP_LAUNCH_PROFILE_NOT_FOUND" => "the ACP Agent is not supported by this release",
            _ => "the ACP Client could not be prepared",
        };
        Self {
            agent_id,
            status: AcpClientReadinessStatus::Unavailable,
            reason_code: Some(error.code),
            reason: Some(reason.to_owned()),
            retryable: error.retryable,
        }
    }
}

impl From<rambledesk_core::kernel::CoreError> for AcpWorkbenchError {
    fn from(error: rambledesk_core::kernel::CoreError) -> Self {
        Self::new(error.code_str(), error.message(), error.retryable())
    }
}
