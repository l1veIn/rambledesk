use super::{
    Core, CoreError, FactMutation, FactMutationOutcome, FactQuery, FactQueryOutcome,
    SessionOrganization, SessionOrganizationCommit, SessionRecord,
    core_support::{now, unexpected_store_outcome, validate_id, validate_nonblank},
};

impl Core {
    pub async fn organize_session(
        &self,
        mutation: SessionOrganization,
    ) -> Result<SessionRecord, CoreError> {
        validate_id("session_id", mutation.session_id().as_str())?;
        if let SessionOrganization::Rename { title, .. } = &mutation {
            validate_nonblank("title", title, 1, 160)?;
        }
        match self
            .facts
            .apply(FactMutation::SessionOrganization(Box::new(
                SessionOrganizationCommit {
                    mutation,
                    now: now()?,
                },
            )))
            .await?
        {
            FactMutationOutcome::SessionOrganization(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }

    pub async fn read_archived_sessions(&self) -> Result<Vec<SessionRecord>, CoreError> {
        match self.facts.query(FactQuery::ArchivedSessions).await? {
            FactQueryOutcome::ArchivedSessions(value) => Ok(value),
            _ => Err(unexpected_store_outcome()),
        }
    }
}
