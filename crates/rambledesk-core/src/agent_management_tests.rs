use super::*;
use crate::{
    AgentConnectionKind, AgentDistribution, AgentInstallObserver, AgentVerification,
    AgentVerificationStatus, NoopApplicationChangeObserver, SaveAgentConfigInput, SessionProtocol,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::Notify;

#[derive(Default)]
struct Provider {
    calls: AtomicUsize,
    registered: AtomicBool,
    started: Notify,
    cancelled: Notify,
    cleaning: Notify,
    finish: Notify,
}
fn entry() -> AgentCatalogEntry {
    AgentCatalogEntry {
        id: "fixture".into(),
        name: "Fixture".into(),
        host_id: "fixture".into(),
        description: String::new(),
        connection_kind: AgentConnectionKind::Native,
        distribution: AgentDistribution::Manual {
            command: "fixture".into(),
            version: "1".into(),
            instructions: String::new(),
            docs_url: String::new(),
        },
        args: vec![],
        dependencies: vec![],
        verification: AgentVerification {
            status: AgentVerificationStatus::Unverified,
            versions: vec![],
            note: String::new(),
        },
    }
}
fn installed() -> InstalledAgent {
    InstalledAgent {
        agent_id: "fixture".into(),
        version: "1".into(),
        config: SaveAgentConfigInput {
            catalog_id: None,
            id: None,
            name: "Fixture".into(),
            host_id: "fixture".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "fixture".into(),
            args: vec![],
            env: Default::default(),
        },
    }
}
#[async_trait::async_trait]
impl AgentCatalogProvider for Provider {
    fn catalog(&self) -> Vec<AgentCatalogEntry> {
        vec![entry()]
    }
    async fn inspect(&self, _: &str) -> Result<AgentInspection, AgentDriverError> {
        unreachable!()
    }
    async fn install(
        &self,
        _: InstallAgentInput,
        progress: AgentInstallObserver,
    ) -> Result<InstalledAgent, AgentDriverError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.registered.store(true, Ordering::SeqCst);
        self.started.notify_one();
        progress(AgentInstallProgress {
            phase: AgentInstallPhase::Installing,
            message: "Started".into(),
        });
        tokio::select! {
            _ = self.finish.notified() => Ok(installed()),
            _ = self.cancelled.notified() => {
                // A descriptive terminal event must not expose a completed job
                // while child processes / incomplete files are still owned.
                progress(AgentInstallProgress { phase: AgentInstallPhase::Cancelled, message: "Cleaning".into() });
                self.cleaning.notify_one();
                self.finish.notified().await;
                Err(AgentDriverError::new("Cancelled"))
            }
        }
    }
    async fn cancel_install(&self, _: &str) -> Result<(), AgentDriverError> {
        if self.registered.load(Ordering::SeqCst) {
            self.cancelled.notify_one();
        }
        Ok(())
    }
}
fn setup() -> (AgentManagementApplication, Arc<Provider>) {
    let provider = Arc::new(Provider::default());
    (
        AgentManagementApplication::new(provider.clone(), Arc::new(NoopApplicationChangeObserver)),
        provider,
    )
}
fn input() -> InstallAgentInput {
    InstallAgentInput {
        agent_id: "fixture".into(),
        version: None,
    }
}
async fn settled(app: &AgentManagementApplication) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while app.jobs().iter().any(AgentInstallJob::is_active) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("job should finish");
}

#[tokio::test]
async fn duplicate_clients_share_the_job_and_only_publish_after_install_finishes() {
    let (app, provider) = setup();
    let first = app.start_install(input()).unwrap();
    assert_eq!(first.id, app.clone().start_install(input()).unwrap().id);
    provider.started.notified().await;
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert!(app.jobs()[0].is_active());
    provider.finish.notify_one();
    settled(&app).await;
    assert_eq!(app.jobs()[0].phase, AgentInstallPhase::Complete);
    assert!(app.jobs()[0].result.is_some());
}

#[tokio::test]
async fn cancelling_before_provider_registration_is_retained_until_cleanup_finishes() {
    let (app, provider) = setup();
    let job = app.start_install(input()).unwrap();
    app.cancel(AgentInstallJobInput { job_id: job.id })
        .await
        .unwrap();
    assert!(!provider.registered.load(Ordering::SeqCst));
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        provider.cleaning.notified(),
    )
    .await
    .unwrap();
    assert!(app.jobs()[0].is_active());
    assert!(app.jobs()[0].cancel_requested);
    provider.finish.notify_one();
    settled(&app).await;
    assert_eq!(app.jobs()[0].phase, AgentInstallPhase::Cancelled);
    assert!(app.jobs()[0].result.is_none());
}

#[tokio::test]
async fn shutdown_blocks_new_jobs_and_waits_for_owned_cleanup() {
    let (app, provider) = setup();
    app.start_install(input()).unwrap();
    provider.started.notified().await;
    let copy = app.clone();
    let shutdown = tokio::spawn(async move { copy.shutdown().await });
    provider.cleaning.notified().await;
    assert!(app.start_install(input()).is_err());
    assert!(!shutdown.is_finished());
    provider.finish.notify_one();
    shutdown.await.unwrap();
    assert_eq!(app.jobs()[0].phase, AgentInstallPhase::Cancelled);
}
