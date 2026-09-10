use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ts_rs::TS;

use super::{ManagedSessionInput, SessionApplication, SessionError, SessionManagement};

/// Optional display metadata. It never loads activity or starts an Agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ManagedWorkspaceInfo {
    pub cwd: String,
    /// Current branch, or a short commit id for a detached HEAD.
    pub branch: Option<String>,
}

#[async_trait]
pub trait WorkspaceInfoProvider: Send + Sync {
    /// Read the server-owned working directory; unavailable metadata is normal.
    async fn branch(&self, cwd: &str) -> Option<String>;
}

impl SessionApplication {
    pub fn with_workspace_info_provider(
        mut self,
        provider: Arc<dyn WorkspaceInfoProvider>,
    ) -> Self {
        self.workspace_info = Some(provider);
        self
    }

    pub async fn get_workspace_info(
        &self,
        input: ManagedSessionInput,
    ) -> Result<ManagedWorkspaceInfo, SessionError> {
        let record = self.managed_record(&input.session_id).await?;
        let SessionManagement::Managed { cwd, .. } = record.management else {
            return Err(SessionError::NotManaged);
        };
        let branch = match &self.workspace_info {
            Some(provider) => provider.branch(&cwd).await,
            None => None,
        };
        Ok(ManagedWorkspaceInfo { cwd, branch })
    }
}
