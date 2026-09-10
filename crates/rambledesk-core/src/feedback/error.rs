use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub enum ApplicationErrorCode {
    #[serde(rename = "INVALID_ARGUMENT")]
    #[ts(rename = "INVALID_ARGUMENT")]
    InvalidArgument,
    #[serde(rename = "REQUEST_NOT_FOUND")]
    #[ts(rename = "REQUEST_NOT_FOUND")]
    RequestNotFound,
    #[serde(rename = "RECOVERY_AMBIGUOUS")]
    #[ts(rename = "RECOVERY_AMBIGUOUS")]
    RecoveryAmbiguous,
    #[serde(rename = "REQUEST_CONFLICT")]
    #[ts(rename = "REQUEST_CONFLICT")]
    RequestConflict,
    #[serde(rename = "REQUEST_ALREADY_COMPLETED")]
    #[ts(rename = "REQUEST_ALREADY_COMPLETED")]
    RequestAlreadyCompleted,
    #[serde(rename = "REQUEST_TERMINAL")]
    #[ts(rename = "REQUEST_TERMINAL")]
    RequestTerminal,
    #[serde(rename = "DRAFT_CONFLICT")]
    #[ts(rename = "DRAFT_CONFLICT")]
    DraftConflict,
    #[serde(rename = "ATTACHMENT_NOT_FOUND")]
    #[ts(rename = "ATTACHMENT_NOT_FOUND")]
    AttachmentNotFound,
    #[serde(rename = "ATTACHMENT_LIMIT")]
    #[ts(rename = "ATTACHMENT_LIMIT")]
    AttachmentLimit,
    #[serde(rename = "HOST_SESSION_NOT_FOUND")]
    #[ts(rename = "HOST_SESSION_NOT_FOUND")]
    HostSessionNotFound,
    #[serde(rename = "HOST_SESSION_HAS_OPEN_REQUESTS")]
    #[ts(rename = "HOST_SESSION_HAS_OPEN_REQUESTS")]
    HostSessionHasOpenRequests,
    #[serde(rename = "DELETE_REQUIRES_ARCHIVED_HOST_SESSION")]
    #[ts(rename = "DELETE_REQUIRES_ARCHIVED_HOST_SESSION")]
    DeleteRequiresArchivedHostSession,
    #[serde(rename = "REQUEST_NOT_TERMINAL")]
    #[ts(rename = "REQUEST_NOT_TERMINAL")]
    RequestNotTerminal,
    #[serde(rename = "PACKAGE_PUBLISH_FAILURE")]
    #[ts(rename = "PACKAGE_PUBLISH_FAILURE")]
    PackagePublishFailure,
    #[serde(rename = "FEEDBACK_PACKAGE_READ_FAILURE")]
    #[ts(rename = "FEEDBACK_PACKAGE_READ_FAILURE")]
    FeedbackPackageReadFailure,
    #[serde(rename = "STORAGE_FAILURE")]
    #[ts(rename = "STORAGE_FAILURE")]
    StorageFailure,
}

impl ApplicationErrorCode {
    pub const ALL: [Self; 16] = [
        Self::InvalidArgument,
        Self::RequestNotFound,
        Self::RecoveryAmbiguous,
        Self::RequestConflict,
        Self::RequestAlreadyCompleted,
        Self::RequestTerminal,
        Self::DraftConflict,
        Self::AttachmentNotFound,
        Self::AttachmentLimit,
        Self::HostSessionNotFound,
        Self::HostSessionHasOpenRequests,
        Self::DeleteRequiresArchivedHostSession,
        Self::RequestNotTerminal,
        Self::PackagePublishFailure,
        Self::FeedbackPackageReadFailure,
        Self::StorageFailure,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::RequestNotFound => "REQUEST_NOT_FOUND",
            Self::RecoveryAmbiguous => "RECOVERY_AMBIGUOUS",
            Self::RequestConflict => "REQUEST_CONFLICT",
            Self::RequestAlreadyCompleted => "REQUEST_ALREADY_COMPLETED",
            Self::RequestTerminal => "REQUEST_TERMINAL",
            Self::DraftConflict => "DRAFT_CONFLICT",
            Self::AttachmentNotFound => "ATTACHMENT_NOT_FOUND",
            Self::AttachmentLimit => "ATTACHMENT_LIMIT",
            Self::HostSessionNotFound => "HOST_SESSION_NOT_FOUND",
            Self::HostSessionHasOpenRequests => "HOST_SESSION_HAS_OPEN_REQUESTS",
            Self::DeleteRequiresArchivedHostSession => "DELETE_REQUIRES_ARCHIVED_HOST_SESSION",
            Self::RequestNotTerminal => "REQUEST_NOT_TERMINAL",
            Self::PackagePublishFailure => "PACKAGE_PUBLISH_FAILURE",
            Self::FeedbackPackageReadFailure => "FEEDBACK_PACKAGE_READ_FAILURE",
            Self::StorageFailure => "STORAGE_FAILURE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, JsonSchema, TS)]
#[error("{message}")]
pub struct ApplicationError {
    code: ApplicationErrorCode,
    message: String,
    retryable: bool,
}

impl ApplicationError {
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self {
            code: ApplicationErrorCode::InvalidArgument,
            message: message.into(),
            retryable: false,
        }
    }

    pub(super) fn request_not_found() -> Self {
        Self {
            code: ApplicationErrorCode::RequestNotFound,
            message: "feedback request was not found for this host session".to_owned(),
            retryable: false,
        }
    }

    pub(super) fn recovery_ambiguous() -> Self {
        Self {
            code: ApplicationErrorCode::RecoveryAmbiguous,
            message: "multiple feedback requests match this host session; provide request_id"
                .to_owned(),
            retryable: false,
        }
    }

    pub fn code(&self) -> &'static str {
        self.code.as_str()
    }

    pub const fn code_enum(&self) -> ApplicationErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

impl From<RepositoryError> for ApplicationError {
    fn from(value: RepositoryError) -> Self {
        let (code, message, retryable) = match value {
            RepositoryError::RequestNotFound => (
                ApplicationErrorCode::RequestNotFound,
                "feedback request was not found",
                false,
            ),
            RepositoryError::RequestConflict => (
                ApplicationErrorCode::RequestConflict,
                "request_id already exists with different immutable input",
                false,
            ),
            RepositoryError::RequestAlreadyCompleted => (
                ApplicationErrorCode::RequestAlreadyCompleted,
                "completed feedback cannot be cancelled",
                false,
            ),
            RepositoryError::RequestTerminal => (
                ApplicationErrorCode::RequestTerminal,
                "terminal feedback cannot be modified",
                false,
            ),
            RepositoryError::DraftConflict => (
                ApplicationErrorCode::DraftConflict,
                "draft revision changed; reload before saving or submitting",
                false,
            ),
            RepositoryError::DraftEmpty => (
                ApplicationErrorCode::InvalidArgument,
                "feedback draft cannot be empty when submitting",
                false,
            ),
            RepositoryError::AttachmentNotFound => (
                ApplicationErrorCode::AttachmentNotFound,
                "feedback attachment was not found",
                false,
            ),
            RepositoryError::AttachmentLimit => (
                ApplicationErrorCode::AttachmentLimit,
                "a feedback request can contain at most 20 attachments",
                false,
            ),
            RepositoryError::HostSessionNotFound => (
                ApplicationErrorCode::HostSessionNotFound,
                "host session was not found",
                false,
            ),
            RepositoryError::HostSessionHasOpenRequests => (
                ApplicationErrorCode::HostSessionHasOpenRequests,
                "finish or cancel open feedback requests before archiving this session",
                false,
            ),
            RepositoryError::DeleteRequiresArchivedHostSession => (
                ApplicationErrorCode::DeleteRequiresArchivedHostSession,
                "archive the host session before deleting requests",
                false,
            ),
            RepositoryError::ManagedSessionRequiresRuntimeDeletion => (
                ApplicationErrorCode::InvalidArgument,
                "delete managed sessions through the managed session command so runtime resources are stopped first",
                false,
            ),
            RepositoryError::RequestNotTerminal => (
                ApplicationErrorCode::RequestNotTerminal,
                "finish or cancel the feedback request before deleting it",
                false,
            ),
            RepositoryError::PackagePublish => (
                ApplicationErrorCode::PackagePublishFailure,
                "feedback package could not be published",
                true,
            ),
            RepositoryError::PackageRead => (
                ApplicationErrorCode::FeedbackPackageReadFailure,
                "feedback package could not be read or verified",
                true,
            ),
            RepositoryError::CorruptData | RepositoryError::Storage => (
                ApplicationErrorCode::StorageFailure,
                "feedback storage operation failed",
                true,
            ),
        };
        Self {
            code,
            message: message.to_owned(),
            retryable,
        }
    }
}
