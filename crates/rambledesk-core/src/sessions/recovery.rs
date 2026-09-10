use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::SessionRepositoryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum SessionRecoveryStatus {
    NeverStarted,
    // Historical checkpoint, not evidence of a live connection in this runtime.
    Unclosed,
    Stopped,
    Interrupted,
}

impl TryFrom<&str> for SessionRecoveryStatus {
    type Error = SessionRepositoryError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "never_started" => Ok(Self::NeverStarted),
            "unclosed" => Ok(Self::Unclosed),
            "stopped" => Ok(Self::Stopped),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(SessionRepositoryError::CorruptData),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRunEnd {
    Stopped,
    Interrupted,
}

impl SessionRunEnd {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SessionRecovery {
    pub session_id: String,
    pub status: SessionRecoveryStatus,
    pub run_id: Option<String>,
    pub active_turn_id: Option<String>,
    pub interrupted_turn_id: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[async_trait]
pub trait SessionRecoveryRepository: Send + Sync {
    async fn get_session_recovery(
        &self,
        session_id: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError>;

    /// Persist before launching. A new run cannot replace an unclosed checkpoint;
    /// explicitly close it or perform startup recovery first. Repeating the same
    /// run id is idempotent while it remains unclosed.
    async fn begin_run(
        &self,
        session_id: &str,
        run_id: &str,
        now: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError>;

    /// Persist before sending the prompt. Only one turn may be open per run.
    async fn begin_turn(
        &self,
        session_id: &str,
        run_id: &str,
        turn_id: &str,
        now: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError>;

    /// Complete the matching checkpoint after recording the terminal turn activity.
    /// The application determines whether the turn completed, failed or cancelled.
    async fn finish_turn(
        &self,
        session_id: &str,
        run_id: &str,
        turn_id: &str,
        now: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError>;

    /// Both explicit stops and failures leave an unfinished turn visibly interrupted.
    /// Old run ids cannot close or overwrite the checkpoint of a replacement run.
    async fn close_run(
        &self,
        session_id: &str,
        run_id: &str,
        end: SessionRunEnd,
        last_error: Option<&str>,
        now: &str,
    ) -> Result<SessionRecovery, SessionRepositoryError>;

    /// Call once at application startup, before launching any new runs. Atomically
    /// closes historical unclosed checkpoints and records interruption activities.
    async fn recover_open_runs(
        &self,
        now: &str,
    ) -> Result<Vec<SessionRecovery>, SessionRepositoryError>;
}
