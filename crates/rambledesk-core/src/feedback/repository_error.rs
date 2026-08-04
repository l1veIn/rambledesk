use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RepositoryError {
    #[error("feedback request was not found")]
    RequestNotFound,
    #[error("feedback request conflicts with an existing request")]
    RequestConflict,
    #[error("feedback request is already completed")]
    RequestAlreadyCompleted,
    #[error("feedback request is terminal")]
    RequestTerminal,
    #[error("draft revision conflicts with the stored revision")]
    DraftConflict,
    #[error("feedback draft is empty")]
    DraftEmpty,
    #[error("attachment was not found")]
    AttachmentNotFound,
    #[error("attachment limit was reached")]
    AttachmentLimit,
    #[error("feedback package publication failed")]
    PackagePublish,
    #[error("feedback package could not be read")]
    PackageRead,
    #[error("stored feedback data is invalid")]
    CorruptData,
    #[error("storage operation failed")]
    Storage,
}
