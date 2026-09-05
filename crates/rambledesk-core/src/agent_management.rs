//! Application-owned installation jobs shared by Desktop and Web clients.
mod catalog_resolution;
#[cfg(test)]
#[path = "agent_management_tests.rs"]
mod tests;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::{
    AgentCatalogEntry, AgentCatalogProvider, AgentDriverError, AgentInspection, AgentInstallPhase,
    AgentInstallProgress, ApplicationChange, ApplicationChangeObserver, ApplicationResourceKey,
    InstallAgentInput, InstalledAgent,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CatalogAgentInput {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResolveCatalogAgentInput {
    pub agent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent_config_id: Option<String>,
    #[serde(default)]
    pub enable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AgentInstallJobInput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AgentInstallJob {
    pub id: String,
    pub agent_id: String,
    pub phase: AgentInstallPhase,
    pub messages: Vec<String>,
    pub result: Option<InstalledAgent>,
    pub cancel_requested: bool,
}

impl AgentInstallJob {
    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            AgentInstallPhase::Preparing
                | AgentInstallPhase::Installing
                | AgentInstallPhase::Verifying
        )
    }
}

struct State {
    jobs: BTreeMap<String, AgentInstallJob>,
    shutting_down: bool,
}

#[derive(Clone)]
pub struct AgentManagementApplication {
    provider: Arc<dyn AgentCatalogProvider>,
    state: Arc<Mutex<State>>,
    changes: Arc<dyn ApplicationChangeObserver>,
    pub(crate) catalog_resolution: Arc<tokio::sync::Mutex<()>>,
}

impl AgentManagementApplication {
    pub fn new(
        provider: Arc<dyn AgentCatalogProvider>,
        changes: Arc<dyn ApplicationChangeObserver>,
    ) -> Self {
        Self {
            provider,
            state: Arc::new(Mutex::new(State {
                jobs: BTreeMap::new(),
                shutting_down: false,
            })),
            changes,
            catalog_resolution: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn catalog(&self) -> Vec<AgentCatalogEntry> {
        self.provider.catalog()
    }

    pub async fn inspect(
        &self,
        input: CatalogAgentInput,
    ) -> Result<AgentInspection, AgentDriverError> {
        self.provider.inspect(&input.agent_id).await
    }

    pub fn jobs(&self) -> Vec<AgentInstallJob> {
        self.state
            .lock()
            .expect("installation jobs")
            .jobs
            .values()
            .cloned()
            .collect()
    }

    pub fn start_install(
        &self,
        input: InstallAgentInput,
    ) -> Result<AgentInstallJob, AgentDriverError> {
        if !self
            .catalog()
            .iter()
            .any(|entry| entry.id == input.agent_id)
        {
            return Err(AgentDriverError::new("Unknown agent"));
        }
        let job = AgentInstallJob {
            id: uuid::Uuid::now_v7().to_string(),
            agent_id: input.agent_id.clone(),
            phase: AgentInstallPhase::Preparing,
            messages: vec![],
            result: None,
            cancel_requested: false,
        };
        {
            let mut state = self.state.lock().expect("installation jobs");
            if state.shutting_down {
                return Err(AgentDriverError::new("Agent management is shutting down"));
            }
            if let Some(existing) = state
                .jobs
                .values()
                .find(|job| job.agent_id == input.agent_id && job.is_active())
            {
                return Ok(existing.clone());
            }
            while state.jobs.len() >= 64 {
                let old = state
                    .jobs
                    .iter()
                    .find(|(_, job)| !job.is_active())
                    .map(|(id, _)| id.clone());
                if let Some(old) = old {
                    state.jobs.remove(&old);
                } else {
                    return Err(AgentDriverError::new("Too many active installations"));
                }
            }
            state.jobs.insert(job.id.clone(), job.clone());
        }
        let this = self.clone();
        let job_id = job.id.clone();
        tokio::spawn(async move {
            // Keep the job active until the provider has actually cleaned up;
            // progress is descriptive and cannot publish an early terminal state.
            let progress_owner = this.clone();
            let progress_id = job_id.clone();
            let progress = Arc::new(move |progress: AgentInstallProgress| {
                let mut state = progress_owner.state.lock().expect("installation jobs");
                if let Some(job) = state.jobs.get_mut(&progress_id) {
                    if matches!(
                        progress.phase,
                        AgentInstallPhase::Preparing
                            | AgentInstallPhase::Installing
                            | AgentInstallPhase::Verifying
                    ) {
                        job.phase = progress.phase;
                    }
                    let message: String = progress.message.chars().take(1024).collect();
                    if job.messages.last() != Some(&message) {
                        job.messages.push(message);
                    }
                    if job.messages.len() > 80 {
                        job.messages.remove(0);
                    }
                }
            });
            let agent_id = input.agent_id.clone();
            let installation = this.provider.install(input, progress);
            tokio::pin!(installation);
            let mut cancellation_check =
                tokio::time::interval(std::time::Duration::from_millis(50));
            let result = loop {
                tokio::select! {
                    result = &mut installation => break result,
                    _ = cancellation_check.tick() => {
                        let cancelled = this.state.lock().expect("installation jobs")
                            .jobs.get(&job_id).is_some_and(|job| job.cancel_requested);
                        if cancelled {
                            // Retain cancellation intent even if the client cancelled
                            // before the provider registered its process/token.
                            let _ = this.provider.cancel_install(&agent_id).await;
                        }
                    }
                }
            };
            {
                let mut state = this.state.lock().expect("installation jobs");
                if let Some(job) = state.jobs.get_mut(&job_id) {
                    match result {
                        Ok(installed) => {
                            job.phase = AgentInstallPhase::Complete;
                            job.result = Some(installed);
                        }
                        Err(error) => {
                            job.phase = if job.cancel_requested {
                                AgentInstallPhase::Cancelled
                            } else {
                                AgentInstallPhase::Failed
                            };
                            job.messages
                                .push(error.message.chars().take(1024).collect());
                        }
                    }
                }
            }
            this.changed();
        });
        self.changed();
        Ok(job)
    }

    pub async fn cancel(&self, input: AgentInstallJobInput) -> Result<(), AgentDriverError> {
        let agent_id = {
            let mut state = self.state.lock().expect("installation jobs");
            let job = state
                .jobs
                .get_mut(&input.job_id)
                .ok_or_else(|| AgentDriverError::new("Installation job not found"))?;
            if !job.is_active() {
                return Ok(());
            }
            job.cancel_requested = true;
            job.agent_id.clone()
        };
        self.provider.cancel_install(&agent_id).await?;
        self.changed();
        Ok(())
    }

    pub async fn shutdown(&self) {
        let active = {
            let mut state = self.state.lock().expect("installation jobs");
            state.shutting_down = true;
            state
                .jobs
                .values()
                .filter(|job| job.is_active())
                .map(|job| job.id.clone())
                .collect::<Vec<_>>()
        };
        for job_id in active {
            let _ = self.cancel(AgentInstallJobInput { job_id }).await;
        }
        let _ = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while self.jobs().iter().any(AgentInstallJob::is_active) {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
    }

    fn changed(&self) {
        self.changes.observe(ApplicationChange {
            resources: vec![ApplicationResourceKey::AgentConfigurations],
        });
    }
}
