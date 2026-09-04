use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::SessionRepositoryError;

#[path = "activity_content.rs"]
mod content;
pub use content::*;

pub const MAX_SESSION_ACTIVITY_PAGE_SIZE: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SessionActivityKind {
    UserMessage,
    AgentMessage,
    AgentThought,
    ToolCall,
    Status,
    Error,
}

impl SessionActivityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserMessage => "user_message",
            Self::AgentMessage => "agent_message",
            Self::AgentThought => "agent_thought",
            Self::ToolCall => "tool_call",
            Self::Status => "status",
            Self::Error => "error",
        }
    }
}

impl TryFrom<&str> for SessionActivityKind {
    type Error = SessionRepositoryError;

    fn try_from(value: &str) -> Result<Self, SessionRepositoryError> {
        match value {
            "user_message" => Ok(Self::UserMessage),
            "agent_message" => Ok(Self::AgentMessage),
            "agent_thought" => Ok(Self::AgentThought),
            "tool_call" => Ok(Self::ToolCall),
            "status" => Ok(Self::Status),
            "error" => Ok(Self::Error),
            _ => Err(SessionRepositoryError::CorruptData),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionActivity {
    pub id: String,
    pub session_id: String,
    #[ts(type = "number")]
    pub sequence: u64,
    pub turn_id: Option<String>,
    pub kind: SessionActivityKind,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub content: Option<SessionActivityContent>,
    pub tool_call_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSessionActivity {
    pub id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub kind: SessionActivityKind,
    pub text: String,
    pub content: Option<SessionActivityContent>,
    pub tool_call_id: Option<String>,
    pub created_at: String,
}

#[async_trait]
pub trait SessionActivityRepository: Send + Sync {
    /// Allocates a sequence within the target session atomically. Reusing an id
    /// with identical content is idempotent; different content is a conflict.
    async fn append_activity(
        &self,
        activity: NewSessionActivity,
    ) -> Result<SessionActivity, SessionRepositoryError>;

    /// The cursor addresses newly appended rows, not edits to previous rows.
    /// Clients receiving an invalidation must refetch the relevant snapshot to
    /// observe in-place streaming updates made by update_activity_text.
    async fn list_session_activity(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
        limit: u32,
    ) -> Result<Vec<SessionActivity>, SessionRepositoryError>;

    /// Returns the most recent bounded window in ascending sequence order. Use
    /// this for live snapshots, so long conversations keep showing new activity.
    async fn list_recent_session_activity(
        &self,
        session_id: &str,
        limit: u32,
    ) -> Result<Vec<SessionActivity>, SessionRepositoryError>;

    /// Older immutable window, exclusive of the cursor, in ascending order.
    async fn list_session_activity_before(
        &self,
        session_id: &str,
        before_sequence: u64,
        limit: u32,
    ) -> Result<Vec<SessionActivity>, SessionRepositoryError>;

    /// Replaces the entire text after verifying local session ownership. The
    /// application serializes streaming aggregation before calling this method.
    async fn update_activity_text(
        &self,
        id: &str,
        session_id: &str,
        text: &str,
    ) -> Result<SessionActivity, SessionRepositoryError>;

    /// Replaces a consistent display summary and typed content together after
    /// verifying session ownership. Aggregation is serialized by the application.
    async fn update_activity_content(
        &self,
        id: &str,
        session_id: &str,
        text: &str,
        content: &SessionActivityContent,
    ) -> Result<SessionActivity, SessionRepositoryError>;
}
