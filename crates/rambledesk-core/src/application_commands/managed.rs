use serde::Serialize;
use thiserror::Error;

use super::ApplicationCommandFacade;
use crate::{
    AgentConfig, AgentConfigInput, AgentConnectionCheck, CreateManagedSessionInput,
    ManagedSessionInput, ManagedSessionSnapshot, RespondManagedPermissionInput,
    SaveAgentConfigInput, SendManagedPromptInput, SessionApplication, SessionError,
    SessionRepositoryError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedCommandErrorCode {
    ManagedRuntimeUnavailable,
    ManagedSessionNotFound,
    AgentConfigNotFound,
    AgentConfigInUse,
    AgentConfigDisabled,
    InvalidArgument,
    ManagedSessionConflict,
    StorageFailure,
    AgentOperationFailed,
    ManagedSessionBusy,
    ManagedSessionNotConnected,
    ManagedSessionInterrupted,
    RuntimeShuttingDown,
    SessionNotManaged,
}

/// Transport-neutral error contract. Driver failures are mapped to a stable
/// diagnostic so transport adapters never expose subprocess/protocol internals.
#[derive(Debug, Clone, Serialize, Error)]
#[error("{message}")]
pub struct ManagedCommandError {
    pub code: ManagedCommandErrorCode,
    pub message: &'static str,
    pub retryable: bool,
}

impl From<SessionError> for ManagedCommandError {
    fn from(error: SessionError) -> Self {
        use ManagedCommandErrorCode as Code;
        let (code, message, retryable) = match error {
            SessionError::Repository(error) => match error {
                SessionRepositoryError::SessionNotFound => (
                    Code::ManagedSessionNotFound,
                    "Managed session was not found",
                    false,
                ),
                SessionRepositoryError::AgentConfigNotFound => (
                    Code::AgentConfigNotFound,
                    "Agent configuration was not found",
                    false,
                ),
                SessionRepositoryError::AgentConfigInUse => (
                    Code::AgentConfigInUse,
                    "Agent configuration is referenced by a session",
                    false,
                ),
                SessionRepositoryError::AgentConfigDisabled => (
                    Code::AgentConfigDisabled,
                    "Agent configuration is disabled",
                    false,
                ),
                SessionRepositoryError::InvalidInput => (
                    Code::InvalidArgument,
                    "Managed session input is invalid",
                    false,
                ),
                SessionRepositoryError::Conflict => (
                    Code::ManagedSessionConflict,
                    "Managed session or configuration conflicts with stored data",
                    false,
                ),
                SessionRepositoryError::CorruptData | SessionRepositoryError::Storage => (
                    Code::StorageFailure,
                    "Managed session storage operation failed",
                    false,
                ),
            },
            SessionError::Driver(_) => {
                (Code::AgentOperationFailed, "Agent operation failed", false)
            }
            SessionError::Busy => (Code::ManagedSessionBusy, "Managed session is busy", false),
            SessionError::NotConnected => (
                Code::ManagedSessionNotConnected,
                "Managed session is not connected",
                false,
            ),
            SessionError::Interrupted => (
                Code::ManagedSessionInterrupted,
                "Managed session operation was interrupted",
                false,
            ),
            SessionError::ShuttingDown => (
                Code::RuntimeShuttingDown,
                "Session management is shutting down",
                false,
            ),
            SessionError::NotManaged => (
                Code::SessionNotManaged,
                "Operation requires a managed session",
                false,
            ),
            SessionError::InvalidInput => (
                Code::InvalidArgument,
                "Managed session input is invalid",
                false,
            ),
        };
        Self {
            code,
            message,
            retryable,
        }
    }
}

impl ApplicationCommandFacade {
    pub fn with_sessions(mut self, sessions: SessionApplication) -> Self {
        self.sessions = Some(sessions);
        self
    }

    fn managed_sessions(&self) -> Result<&SessionApplication, ManagedCommandError> {
        self.sessions.as_ref().ok_or(ManagedCommandError {
            code: ManagedCommandErrorCode::ManagedRuntimeUnavailable,
            message: "Managed session runtime is unavailable",
            retryable: false,
        })
    }

    pub async fn list_agent_configs(&self) -> Result<Vec<AgentConfig>, ManagedCommandError> {
        self.managed_sessions()?
            .list_agent_configs()
            .await
            .map_err(Into::into)
    }
    pub async fn save_agent_config(
        &self,
        input: SaveAgentConfigInput,
    ) -> Result<AgentConfig, ManagedCommandError> {
        self.managed_sessions()?
            .save_agent_config(input)
            .await
            .map_err(Into::into)
    }
    pub async fn delete_agent_config(
        &self,
        input: AgentConfigInput,
    ) -> Result<(), ManagedCommandError> {
        self.managed_sessions()?
            .delete_agent_config(input)
            .await
            .map_err(Into::into)
    }
    pub async fn check_agent_config(
        &self,
        input: AgentConfigInput,
    ) -> Result<AgentConnectionCheck, ManagedCommandError> {
        self.managed_sessions()?
            .check_agent_config(input)
            .await
            .map_err(Into::into)
    }
    pub async fn create_managed_session(
        &self,
        input: CreateManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
        self.managed_sessions()?
            .create_session(input)
            .await
            .map_err(Into::into)
    }
    pub async fn get_managed_session(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
        self.managed_sessions()?
            .get_session(input)
            .await
            .map_err(Into::into)
    }
    pub async fn start_managed_session(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
        self.managed_sessions()?
            .start_session(input)
            .await
            .map_err(Into::into)
    }
    pub async fn stop_managed_session(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
        self.managed_sessions()?
            .stop_session(input)
            .await
            .map_err(Into::into)
    }
    pub async fn send_managed_prompt(
        &self,
        input: SendManagedPromptInput,
    ) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
        self.managed_sessions()?
            .send_prompt(input)
            .await
            .map_err(Into::into)
    }
    pub async fn cancel_managed_prompt(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
        self.managed_sessions()?
            .cancel_prompt(input)
            .await
            .map_err(Into::into)
    }
    pub async fn respond_managed_permission(
        &self,
        input: RespondManagedPermissionInput,
    ) -> Result<ManagedSessionSnapshot, ManagedCommandError> {
        self.managed_sessions()?
            .respond_permission(input)
            .await
            .map_err(Into::into)
    }
}
