//! Catalog and explicitly requested local npm installs. Portions adapted from
//! Codeg 3ebdfed (Apache-2.0); see each module's source attribution and NOTICE.
mod catalog;
mod inspect;
mod install;
mod paths;
mod runner;
mod version;

use async_trait::async_trait;
use rambledesk_core::*;
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub use catalog::catalog;

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("Agent catalog entry does not exist")]
    UnknownAgent,
    #[error("This Agent distribution requires the catalog's manual installation instructions")]
    ManualInstall,
    #[error("Install version must be a concrete numeric release version")]
    InvalidVersion,
    #[error("Agent install was cancelled")]
    Cancelled,
    #[error("An installation for this Agent is already running")]
    Busy,
    #[error("Node.js or the requested command is unavailable")]
    CommandUnavailable,
    #[error("Node.js does not meet this Agent's minimum version")]
    NodeVersion,
    #[error("Agent command failed; check the runtime and registry connectivity")]
    CommandFailed,
    #[error("Agent command timed out and its owned processes were stopped")]
    Timeout,
    #[error("The managed install directory is not writable")]
    PermissionDenied,
    #[error("The requested npm package version is unavailable")]
    VersionUnavailable,
    #[error("npm rejected a proxy URL; configure a complete URL including its scheme")]
    InvalidProxy,
    #[error("Managed install data or entry point is incomplete or invalid")]
    InvalidInstall,
    #[error("The Agent directory must be a dedicated absolute RambleDesk directory")]
    InvalidRoot,
    #[error("Could not update or clean the managed Agent directory")]
    Storage,
}

#[derive(Clone)]
pub struct AgentCatalogService {
    root: PathBuf,
    root_gate: Arc<Mutex<()>>,
    active: Arc<Mutex<HashMap<String, CancellationToken>>>,
    probe_timeout: Duration,
    install_timeout: Duration,
    #[cfg(test)]
    tools: Option<inspect::Toolchain>,
}

impl AgentCatalogService {
    pub fn new(root: PathBuf) -> Result<Self, CatalogError> {
        if !root.is_absolute() || root.parent().is_none() || root.file_name().is_none() {
            return Err(CatalogError::InvalidRoot);
        }
        Ok(Self {
            root,
            root_gate: Arc::new(Mutex::new(())),
            active: Arc::new(Mutex::new(HashMap::new())),
            probe_timeout: Duration::from_secs(10),
            install_timeout: Duration::from_secs(600),
            #[cfg(test)]
            tools: None,
        })
    }
    pub fn catalog(&self) -> Vec<AgentCatalogEntry> {
        catalog()
    }
    pub async fn inspect_with_cancel(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<AgentInspection, CatalogError> {
        self.inspect_inner(id, cancel).await
    }
    pub async fn install_with_cancel(
        &self,
        input: InstallAgentInput,
        cancel: CancellationToken,
        observer: AgentInstallObserver,
    ) -> Result<InstalledAgent, CatalogError> {
        let id = input.agent_id.clone();
        {
            let mut active = self.active.lock().await;
            if active.contains_key(&id) {
                return Err(CatalogError::Busy);
            }
            active.insert(id.clone(), cancel.clone());
        }
        let registration = install::Registration::new(self.active.clone(), id);
        let result = self.install_inner(input, &cancel, observer.clone()).await;
        if let Err(error) = &result {
            observer(AgentInstallProgress {
                phase: if matches!(error, CatalogError::Cancelled) {
                    AgentInstallPhase::Cancelled
                } else {
                    AgentInstallPhase::Failed
                },
                message: error.to_string(),
            });
        }
        registration.remove().await;
        result
    }
}

#[async_trait]
impl AgentCatalogProvider for AgentCatalogService {
    fn catalog(&self) -> Vec<AgentCatalogEntry> {
        self.catalog()
    }
    async fn inspect(&self, id: &str) -> Result<AgentInspection, AgentDriverError> {
        self.inspect_with_cancel(id, &CancellationToken::new())
            .await
            .map_err(|error| AgentDriverError::new(error.to_string()))
    }
    async fn install(
        &self,
        input: InstallAgentInput,
        progress: AgentInstallObserver,
    ) -> Result<InstalledAgent, AgentDriverError> {
        self.install_with_cancel(input, CancellationToken::new(), progress)
            .await
            .map_err(|error| AgentDriverError::new(error.to_string()))
    }
    async fn cancel_install(&self, id: &str) -> Result<(), AgentDriverError> {
        if let Some(token) = self.active.lock().await.get(id) {
            token.cancel();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
