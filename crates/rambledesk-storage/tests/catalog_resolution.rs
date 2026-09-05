use async_trait::async_trait;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct NeverStart;
#[async_trait]
impl AgentSessionDriver for NeverStart {
    async fn start(&self, _: AgentSessionLaunch) -> Result<StartedAgentSession, AgentDriverError> {
        panic!("resolving must not start an Agent")
    }
    async fn check(&self, _: &AgentConfig) -> Result<AgentSessionCapabilities, AgentDriverError> {
        panic!("resolving must not run a handshake")
    }
}

#[derive(Default)]
struct Catalog {
    inspections: AtomicUsize,
}
#[async_trait]
impl AgentCatalogProvider for Catalog {
    fn catalog(&self) -> Vec<AgentCatalogEntry> {
        vec![AgentCatalogEntry {
            id: "pi-acp".into(),
            name: "Pi".into(),
            host_id: "pi".into(),
            description: String::new(),
            connection_kind: AgentConnectionKind::Bridge,
            distribution: AgentDistribution::Manual {
                command: "pi-acp".into(),
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
        }]
    }
    async fn inspect(&self, id: &str) -> Result<AgentInspection, AgentDriverError> {
        self.inspections.fetch_add(1, Ordering::SeqCst);
        Ok(AgentInspection {
            agent_id: id.into(),
            env: Some(BTreeMap::from([("DEFAULT".into(), "retain".into())])),
            source: AgentInstallSource::System,
            version: Some("1".into()),
            command: Some("/installed/pi-acp".into()),
            args: vec!["--catalog-default".into()],
            dependencies: vec![],
            checks: vec![],
        })
    }
    async fn install(
        &self,
        _: InstallAgentInput,
        _: AgentInstallObserver,
    ) -> Result<InstalledAgent, AgentDriverError> {
        panic!("resolving must not install")
    }
    async fn cancel_install(&self, _: &str) -> Result<(), AgentDriverError> {
        unreachable!()
    }
}
fn selection(id: Option<&str>, enable: bool) -> ResolveCatalogAgentInput {
    ResolveCatalogAgentInput {
        agent_id: "pi-acp".into(),
        agent_config_id: id.map(str::to_owned),
        enable,
    }
}

#[tokio::test]
async fn explicit_catalog_resolution_is_atomic_preserves_profiles_and_requires_unambiguous_choice()
{
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteFeedbackStore::connect(&dir.path().join("store.sqlite"))
            .await
            .unwrap(),
    );
    let sessions = SessionApplication::new(store.clone(), store.clone(), Arc::new(NeverStart));
    let provider = Arc::new(Catalog::default());
    let agents =
        AgentManagementApplication::new(provider.clone(), Arc::new(NoopApplicationChangeObserver));
    assert_eq!(agents.catalog().len(), 1);
    assert!(
        sessions.list_agent_configs().await.unwrap().is_empty(),
        "listing must not save configurations"
    );
    let (left, right) = tokio::join!(
        agents.resolve_configuration(&sessions, selection(None, false)),
        agents.resolve_configuration(&sessions, selection(None, false))
    );
    let created = left.unwrap();
    assert_eq!(created, right.unwrap());
    assert_eq!(provider.inspections.load(Ordering::SeqCst), 1);
    assert_eq!(created.catalog_id.as_deref(), Some("pi-acp"));
    assert_eq!(
        created.env.get("DEFAULT").map(String::as_str),
        Some("retain")
    );
    assert!(store.list_managed_sessions().await.unwrap().is_empty());

    let edited = sessions
        .save_agent_config(SaveAgentConfigInput {
            id: Some(created.id.clone()),
            catalog_id: None,
            name: "My Pi".into(),
            host_id: "pi".into(),
            protocol: SessionProtocol::Acp,
            enabled: false,
            command: "/custom/pi-wrapper".into(),
            args: vec!["--my-flag".into()],
            env: [("KEY".into(), "saved-secret".into())].into(),
        })
        .await
        .unwrap();
    assert_eq!(
        edited.catalog_id, created.catalog_id,
        "ordinary editing retains identity"
    );
    assert!(
        agents
            .resolve_configuration(&sessions, selection(None, false))
            .await
            .is_err()
    );
    assert_eq!(
        store.get_agent_config(&edited.id).await.unwrap(),
        edited,
        "disabled lookup does not mutate"
    );
    let enabled = agents
        .resolve_configuration(&sessions, selection(Some(&edited.id), true))
        .await
        .unwrap();
    assert!(enabled.enabled);
    assert_eq!(
        (&enabled.command, &enabled.args, &enabled.env, &enabled.name),
        (&edited.command, &edited.args, &edited.env, &edited.name)
    );
    assert_eq!(
        provider.inspections.load(Ordering::SeqCst),
        1,
        "existing profile must not be replaced from detection"
    );
    let mut other = enabled.clone();
    other.id = "second-profile".into();
    other.name = "Another account".into();
    store.save_agent_config(other.clone()).await.unwrap();
    assert!(
        agents
            .resolve_configuration(&sessions, selection(None, false))
            .await
            .is_err()
    );
    assert_eq!(
        agents
            .resolve_configuration(&sessions, selection(Some(&other.id), false))
            .await
            .unwrap(),
        other
    );
    assert_eq!(sessions.list_agent_configs().await.unwrap().len(), 2);
}
