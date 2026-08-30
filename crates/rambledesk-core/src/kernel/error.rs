use thiserror::Error;

use super::ports::{ArtifactStoreError, FactStoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreErrorCode {
    InvalidArgument,
    IdempotencyConflict,
    SessionNotFound,
    SessionNotManaged,
    AcpSessionLinkNotFound,
    RequestNotFound,
    RequestTerminal,
    DraftConflict,
    ArtifactNotFound,
    ArtifactDigestMismatch,
    WorkNotFound,
    WorkClaimConflict,
    CorruptData,
    StorageFailure,
}

impl CoreErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::IdempotencyConflict => "IDEMPOTENCY_CONFLICT",
            Self::SessionNotFound => "SESSION_NOT_FOUND",
            Self::SessionNotManaged => "SESSION_NOT_MANAGED",
            Self::AcpSessionLinkNotFound => "ACP_SESSION_LINK_NOT_FOUND",
            Self::RequestNotFound => "REQUEST_NOT_FOUND",
            Self::RequestTerminal => "REQUEST_TERMINAL",
            Self::DraftConflict => "DRAFT_CONFLICT",
            Self::ArtifactNotFound => "ARTIFACT_NOT_FOUND",
            Self::ArtifactDigestMismatch => "ARTIFACT_DIGEST_MISMATCH",
            Self::WorkNotFound => "WORK_NOT_FOUND",
            Self::WorkClaimConflict => "WORK_CLAIM_CONFLICT",
            Self::CorruptData => "CORRUPT_DATA",
            Self::StorageFailure => "STORAGE_FAILURE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct CoreError {
    code: CoreErrorCode,
    message: String,
    retryable: bool,
}

impl CoreError {
    pub fn new(code: CoreErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(CoreErrorCode::InvalidArgument, message, false)
    }

    pub const fn code(&self) -> CoreErrorCode {
        self.code
    }

    pub const fn code_str(&self) -> &'static str {
        self.code.as_str()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }
}

impl From<FactStoreError> for CoreError {
    fn from(value: FactStoreError) -> Self {
        match value {
            FactStoreError::IdempotencyConflict => Self::new(
                CoreErrorCode::IdempotencyConflict,
                "the stable id already exists with different content",
                false,
            ),
            FactStoreError::SessionNotFound => Self::new(
                CoreErrorCode::SessionNotFound,
                "the session was not found",
                false,
            ),
            FactStoreError::SessionNotManaged => Self::new(
                CoreErrorCode::SessionNotManaged,
                "the operation requires a managed session",
                false,
            ),
            FactStoreError::AcpSessionLinkNotFound => Self::new(
                CoreErrorCode::AcpSessionLinkNotFound,
                "the ACP session link was not found for this session",
                false,
            ),
            FactStoreError::RequestNotFound => Self::new(
                CoreErrorCode::RequestNotFound,
                "the feedback request was not found",
                false,
            ),
            FactStoreError::RequestTerminal => Self::new(
                CoreErrorCode::RequestTerminal,
                "the feedback request is already terminal",
                false,
            ),
            FactStoreError::DraftConflict => Self::new(
                CoreErrorCode::DraftConflict,
                "the draft revision changed",
                false,
            ),
            FactStoreError::WorkNotFound => Self::new(
                CoreErrorCode::WorkNotFound,
                "the agent work was not found",
                false,
            ),
            FactStoreError::WorkClaimConflict => Self::new(
                CoreErrorCode::WorkClaimConflict,
                "the agent work claim is stale or invalid",
                false,
            ),
            FactStoreError::CorruptData => Self::new(
                CoreErrorCode::CorruptData,
                "stored domain facts are inconsistent",
                false,
            ),
            FactStoreError::Storage => Self::new(
                CoreErrorCode::StorageFailure,
                "the durable fact store failed",
                true,
            ),
        }
    }
}

impl From<ArtifactStoreError> for CoreError {
    fn from(value: ArtifactStoreError) -> Self {
        match value {
            ArtifactStoreError::NotFound => Self::new(
                CoreErrorCode::ArtifactNotFound,
                "artifact content was not found",
                true,
            ),
            ArtifactStoreError::DigestMismatch => Self::new(
                CoreErrorCode::ArtifactDigestMismatch,
                "artifact content failed digest verification",
                false,
            ),
            ArtifactStoreError::Storage => Self::new(
                CoreErrorCode::StorageFailure,
                "the artifact store failed",
                true,
            ),
        }
    }
}
