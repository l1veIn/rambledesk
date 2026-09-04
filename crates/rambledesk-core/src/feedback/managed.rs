use super::*;
use crate::ManagedFeedbackScope;

impl FeedbackApplication {
    /// Trusted attribution for runtime continuation routing. The marker is read
    /// from storage, never inferred from a model-supplied host label.
    pub async fn managed_feedback_session(
        &self,
        request_id: &str,
    ) -> Result<Option<String>, ApplicationError> {
        let request_id = canonical_uuid(request_id, "request_id")?;
        Ok(self
            .repository
            .get_request(&request_id)
            .await
            .map_err(ApplicationError::from)?
            .managed_session_id)
    }

    pub async fn request_managed_feedback(
        &self,
        scope: &ManagedFeedbackScope,
        mut input: RequestFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        // Any model-supplied host/correlation values are replaced before validation
        // and persistence. Storage validates that the trusted triple still exists.
        input.host_id = Some(scope.host_id.clone());
        input.host_session_id = scope.host_session_id.clone();
        self.request_feedback_with_scope(input, Some(&scope.session_id))
            .await
    }

    pub async fn get_managed_feedback(
        &self,
        scope: &ManagedFeedbackScope,
        input: GetFeedbackInput,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        let request_id = canonical_uuid(&input.request_id, "request_id")?;
        let request = self
            .repository
            .get_request(&request_id)
            .await
            .map_err(ApplicationError::from)?;
        if request.managed_session_id.as_deref() != Some(scope.session_id.as_str())
            || request.host_id != scope.host_id
            || request.host_session_id != scope.host_session_id
        {
            return Err(ApplicationError::request_not_found());
        }
        Ok(request.into())
    }

    pub async fn recover_managed_feedback(
        &self,
        scope: &ManagedFeedbackScope,
        request_id: Option<String>,
    ) -> Result<FeedbackRequestView, ApplicationError> {
        if let Some(request_id) = request_id {
            return self
                .get_managed_feedback(scope, GetFeedbackInput { request_id })
                .await;
        }
        // The existing recovery rule rejects ambiguity. It receives only trusted
        // scope identities, then the result is checked against its durable marker.
        let candidate = self
            .recover_feedback(RecoverFeedbackInput {
                request_id: None,
                host_id: Some(scope.host_id.clone()),
                host_session_id: scope.host_session_id.clone(),
            })
            .await?;
        self.get_managed_feedback(
            scope,
            GetFeedbackInput {
                request_id: candidate.request_id,
            },
        )
        .await
    }
}
