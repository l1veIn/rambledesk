use async_trait::async_trait;
use rambledesk_core::{AttachmentPathResolver, RepositoryError};

use super::SqliteFeedbackStore;

/// Attachment path resolution lives in its own module so the shared
/// `sqlite.rs` repository module stays under the module-size gate.
#[async_trait]
impl AttachmentPathResolver for SqliteFeedbackStore {
    async fn resolve_attachment_path(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<String, RepositoryError> {
        self.resolve_attachment_path_impl(request_id, attachment_id)
            .await
    }

    async fn resolve_request_attachment_path(
        &self,
        request_id: &str,
        attachment_id: &str,
    ) -> Result<String, RepositoryError> {
        self.resolve_request_attachment_path_impl(request_id, attachment_id)
            .await
    }
}
