use async_trait::async_trait;

use crate::feedback::RepositoryError;

/// Resolves the stored filesystem path of an attachment so the desktop app can
/// open it with the system default handler. Kept separate from
/// [`crate::FeedbackRepository`] so repository modules stay small.
#[async_trait]
pub trait AttachmentPathResolver: Send + Sync {
    async fn resolve_attachment_path(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<String, RepositoryError>;

    async fn resolve_request_attachment_path(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<String, RepositoryError>;
}
