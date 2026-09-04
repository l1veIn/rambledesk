use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use axum::Router;
use rambledesk_core::*;
use rambledesk_local_server::{REVISION_HEADER, RUNTIME_GENERATION_HEADER, application_router};
use rambledesk_storage::SqliteFeedbackStore;
use serde_json::{Value, json};
use tokio::{net::TcpListener, sync::Notify, task::JoinHandle};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct Driver {
    checks: AtomicUsize,
}

struct Connection {
    closed: AtomicBool,
    observer: Arc<dyn AgentSessionObserver>,
    session_id: String,
    finish: Notify,
}

#[async_trait]
impl AgentSessionConnection for Connection {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
    async fn prompt(&self, text: &str) -> Result<String, AgentDriverError> {
        if text == "permission" {
            self.observer
                .observe(AgentSessionEvent::PermissionRequested(SessionPermission {
                    request_id: "permission-one".into(),
                    session_id: self.session_id.clone(),
                    title: "Review fixture access".into(),
                    details: None,
                    options: vec![SessionPermissionOption {
                        option_id: "allow".into(),
                        name: "Allow once".into(),
                        kind: "allow_once".into(),
                    }],
                }))
                .await?;
        }
        self.finish.notified().await;
        Ok("EndTurn".into())
    }
    async fn cancel(&self) -> Result<(), AgentDriverError> {
        self.finish.notify_one();
        Ok(())
    }
    async fn respond_permission(&self, _: &str, _: Option<&str>) -> Result<(), AgentDriverError> {
        self.finish.notify_one();
        Ok(())
    }
    async fn stop(&self) -> Result<(), AgentDriverError> {
        self.closed.store(true, Ordering::SeqCst);
        self.finish.notify_one();
        Ok(())
    }
}

#[async_trait]
impl AgentSessionDriver for Driver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        let SessionManagement::Managed {
            remote_session_id, ..
        } = &launch.session.management
        else {
            unreachable!()
        };
        Ok(StartedAgentSession {
            remote_session_id: remote_session_id
                .clone()
                .unwrap_or_else(|| format!("remote-{}", launch.session.session_id)),
            connection: Arc::new(Connection {
                closed: AtomicBool::new(false),
                observer: launch.observer,
                session_id: launch.session.session_id,
                finish: Notify::new(),
            }),
            capabilities: AgentSessionCapabilities {
                load_session: true,
                resume_session: true,
                http_mcp: true,
                prompt: AgentPromptCapabilities::default(),
            },
        })
    }
    async fn check(
        &self,
        config: &AgentConfig,
    ) -> Result<AgentSessionCapabilities, AgentDriverError> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        Ok(AgentSessionCapabilities {
            http_mcp: config.command != "no-http-mcp",
            ..Default::default()
        })
    }
}

struct Fixture {
    directory: tempfile::TempDir,
    store: Arc<SqliteFeedbackStore>,
    feedback: FeedbackApplication,
    facade: Arc<ApplicationCommandFacade>,
    sessions: SessionApplication,
    driver: Arc<Driver>,
    client: reqwest::Client,
    url: String,
    cancellation: CancellationToken,
    task: JoinHandle<std::io::Result<()>>,
}

impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let store =
            Arc::new(SqliteFeedbackStore::connect(&directory.path().join("test.sqlite")).await?);
        let changes = Arc::new(ApplicationChangeHub::with_runtime_generation(
            "managed-test",
        ));
        let application = store
            .as_ref()
            .clone()
            .into_application()
            .with_change_observer(changes.clone());
        let driver = Arc::new(Driver::default());
        let sessions = SessionApplication::new(store.clone(), store.clone(), driver.clone())
            .with_change_observer(changes.clone())
            .with_deliveries(store.clone())
            .with_deletions(store.clone());
        let facade = Arc::new(
            ApplicationCommandFacade::new(
                application.clone(),
                WorkbenchTerminalOperations::without_observer(application.clone()),
                vec![],
            )
            .with_sessions(sessions.clone()),
        );
        let router = Router::new().nest("/api", application_router(facade.clone(), changes));
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let url = format!("http://{}/api/application", listener.local_addr()?);
        let cancellation = CancellationToken::new();
        let token = cancellation.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(token.cancelled_owned())
                .await
        });
        Ok(Self {
            directory,
            store,
            feedback: application,
            facade,
            sessions,
            driver,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?,
            url,
            cancellation,
            task,
        })
    }
    fn request(&self, operation: &str, input: Value) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{}/{operation}", self.url))
            .json(&input)
    }
    async fn call(&self, operation: &str, input: Value) -> anyhow::Result<Value> {
        let response = self
            .request(operation, input)
            .header(RUNTIME_GENERATION_HEADER, "managed-test")
            .send()
            .await?
            .error_for_status()?;
        assert_eq!(
            response.headers()[RUNTIME_GENERATION_HEADER],
            "managed-test"
        );
        assert!(response.headers().contains_key(REVISION_HEADER));
        Ok(response.json().await?)
    }
    async fn wait_for(
        &self,
        id: &str,
        predicate: impl Fn(&ManagedSessionSnapshot) -> bool,
    ) -> anyhow::Result<ManagedSessionSnapshot> {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let snapshot = self
                    .facade
                    .get_managed_session(ManagedSessionInput {
                        session_id: id.into(),
                    })
                    .await?;
                if predicate(&snapshot) {
                    return Ok(snapshot);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?
    }
    async fn shutdown(self) -> anyhow::Result<()> {
        self.sessions.shutdown().await?;
        self.cancellation.cancel();
        self.task.await??;
        Ok(())
    }
}

fn config_input() -> SaveAgentConfigInput {
    SaveAgentConfigInput {
        id: None,
        name: "Fixture".into(),
        host_id: "dsh".into(),
        protocol: SessionProtocol::Acp,
        enabled: true,
        command: "fixture".into(),
        args: vec![],
        env: BTreeMap::new(),
    }
}

#[tokio::test]
async fn configuration_check_rejects_missing_feedback_capability_before_creating_a_session()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let config = fixture
        .facade
        .save_agent_config(SaveAgentConfigInput {
            command: "no-http-mcp".into(),
            ..config_input()
        })
        .await?;
    let checked = fixture
        .call("checkAgentConfig", json!({"agent_config_id":config.id}))
        .await?;
    assert_eq!(checked["ok"], false);
    assert!(
        checked["message"]
            .as_str()
            .unwrap()
            .contains("HTTP MCP is unsupported")
    );
    assert!(
        checked["details"][0]
            .as_str()
            .unwrap()
            .contains("HTTP MCP: false")
    );
    assert_eq!(fixture.driver.checks.load(Ordering::SeqCst), 1);
    assert!(
        fixture
            .call("listHostSessions", Value::Null)
            .await?
            .as_array()
            .unwrap()
            .is_empty()
    );
    fixture.shutdown().await
}

#[tokio::test]
async fn managed_http_uses_the_facade_for_configuration_session_prompt_and_permission_operations()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let config = fixture.facade.save_agent_config(config_input()).await?;
    let listed = fixture.call("listAgentConfigs", Value::Null).await?;
    assert_eq!(
        listed,
        serde_json::to_value(fixture.facade.list_agent_configs().await?)?
    );
    let checked = fixture
        .call("checkAgentConfig", json!({"agent_config_id":config.id}))
        .await?;
    assert_eq!(checked["ok"], true);
    let created = fixture.call("createManagedSession", json!({"agent_config_id":config.id,"cwd":fixture.directory.path(),"title":"Transport session"})).await?;
    assert_eq!(created["runtime"]["connection"], "connected");
    let id = created["session"]["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let input = json!({"session_id":id});
    let snapshot = fixture.call("getManagedSession", input.clone()).await?;
    assert_eq!(
        snapshot,
        serde_json::to_value(
            fixture
                .facade
                .get_managed_session(ManagedSessionInput {
                    session_id: id.clone()
                })
                .await?
        )?
    );
    let sent = fixture
        .call(
            "sendManagedPrompt",
            json!({"session_id":id,"text":"permission"}),
        )
        .await?;
    assert_eq!(sent["session"]["session_id"], id);
    fixture
        .wait_for(&id, |snapshot| !snapshot.permissions.is_empty())
        .await?;
    let invalid = fixture
        .request(
            "respondManagedPermission",
            json!({"session_id":id,"request_id":"permission-one","option_id":"unknown"}),
        )
        .header(RUNTIME_GENERATION_HEADER, "managed-test")
        .send()
        .await?;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(invalid.json::<Value>().await?["code"], "INVALID_ARGUMENT");
    fixture
        .call(
            "respondManagedPermission",
            json!({"session_id":id,"request_id":"permission-one","option_id":"allow"}),
        )
        .await?;
    fixture
        .wait_for(&id, |snapshot| {
            snapshot.runtime.activity == SessionActivityState::Idle
        })
        .await?;
    fixture
        .call("sendManagedPrompt", json!({"session_id":id,"text":"hold"}))
        .await?;
    fixture.call("cancelManagedPrompt", input.clone()).await?;
    fixture
        .wait_for(&id, |snapshot| {
            snapshot.runtime.activity == SessionActivityState::Idle
        })
        .await?;
    assert_eq!(
        fixture.call("stopManagedSession", input.clone()).await?["runtime"]["connection"],
        "stopped"
    );
    assert_eq!(
        fixture.call("startManagedSession", input).await?["runtime"]["connection"],
        "connected"
    );
    let unused = fixture
        .call("saveAgentConfig", serde_json::to_value(config_input())?)
        .await?;
    let deleted = fixture
        .request("deleteAgentConfig", json!({"agent_config_id":unused["id"]}))
        .header(RUNTIME_GENERATION_HEADER, "managed-test")
        .send()
        .await?;
    assert_eq!(deleted.status(), reqwest::StatusCode::NO_CONTENT);
    let in_use = fixture
        .request("deleteAgentConfig", json!({"agent_config_id":config.id}))
        .header(RUNTIME_GENERATION_HEADER, "managed-test")
        .send()
        .await?;
    assert_eq!(in_use.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(in_use.json::<Value>().await?["code"], "AGENT_CONFIG_IN_USE");
    let deleted = fixture
        .request("deleteManagedSession", json!({"session_id":id}))
        .header(RUNTIME_GENERATION_HEADER, "managed-test")
        .send()
        .await?;
    assert_eq!(deleted.status(), reqwest::StatusCode::NO_CONTENT);
    let missing = fixture
        .request("getManagedSession", json!({"session_id":id}))
        .send()
        .await?;
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        missing.json::<Value>().await?["code"],
        "MANAGED_SESSION_NOT_FOUND"
    );
    fixture.shutdown().await
}

#[tokio::test]
async fn every_managed_mutation_requires_current_runtime_generation_including_connection_check()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let config = fixture.facade.save_agent_config(config_input()).await?;
    for operation in [
        "saveAgentConfig",
        "deleteAgentConfig",
        "checkAgentConfig",
        "createManagedSession",
        "startManagedSession",
        "stopManagedSession",
        "sendManagedPrompt",
        "setManagedSessionConfig",
        "sendManagedPromptContent",
        "cancelManagedPrompt",
        "respondManagedPermission",
        "resolveFeedbackDelivery",
        "deleteManagedSession",
    ] {
        for generation in [None, Some("old-runtime")] {
            let request = fixture.request(operation, json!({"agent_config_id":config.id}));
            let request = match generation {
                Some(generation) => request.header(RUNTIME_GENERATION_HEADER, generation),
                None => request,
            };
            let response = request.send().await?;
            assert_eq!(
                response.status(),
                reqwest::StatusCode::CONFLICT,
                "{operation}"
            );
            assert_eq!(
                response.headers()[RUNTIME_GENERATION_HEADER],
                "managed-test"
            );
            assert_eq!(
                response.json::<Value>().await?["code"],
                "RUNTIME_GENERATION_STALE"
            );
        }
    }
    assert_eq!(fixture.driver.checks.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.facade.list_agent_configs().await?.len(), 1);
    assert!(
        fixture
            .request("listAgentConfigs", Value::Null)
            .send()
            .await?
            .status()
            .is_success()
    );
    fixture.shutdown().await
}

#[test]
fn tauri_managed_commands_match_http_names_and_delegate_to_the_same_facade() {
    let commands = include_str!("../../../apps/desktop/src-tauri/src/managed_commands.rs");
    let registration = include_str!("../../../apps/desktop/src-tauri/src/lib.rs");
    let routes = include_str!("../src/application_api/managed.rs");
    for (camel, snake) in [
        ("listAgentConfigs", "list_agent_configs"),
        ("saveAgentConfig", "save_agent_config"),
        ("deleteAgentConfig", "delete_agent_config"),
        ("checkAgentConfig", "check_agent_config"),
        ("createManagedSession", "create_managed_session"),
        ("getManagedSession", "get_managed_session"),
        ("startManagedSession", "start_managed_session"),
        ("stopManagedSession", "stop_managed_session"),
        ("sendManagedPrompt", "send_managed_prompt"),
        ("setManagedSessionConfig", "set_managed_session_config"),
        ("sendManagedPromptContent", "send_managed_prompt_content"),
        ("listManagedSessionActivity", "list_managed_session_activity"),
        ("cancelManagedPrompt", "cancel_managed_prompt"),
        ("respondManagedPermission", "respond_managed_permission"),
        ("resolveFeedbackDelivery", "resolve_feedback_delivery"),
        ("deleteManagedSession", "delete_managed_session"),
    ] {
        assert!(commands.contains(&format!("async fn {snake}(")), "{snake}");
        assert!(
            commands.contains(&format!(".{snake}(")),
            "{snake} delegates"
        );
        assert!(
            registration.contains(&format!("managed_commands::{snake},")),
            "{snake} registered"
        );
        assert!(routes.contains(&format!("/application/{camel}")), "{camel}");
    }
    assert_eq!(commands.matches("#[tauri::command]").count(), 16);
    assert_eq!(commands.matches("input:").count(), 15);
    assert!(registration.contains(".with_sessions(sessions.clone())"));
    assert!(registration.contains("state.sessions.shutdown()"));
    assert!(registration.contains("sessions.start_delivery_worker()"));
}

#[tokio::test]
async fn typed_prompt_has_a_bounded_larger_body_without_relaxing_other_commands() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let oversized_image = json!({
        "session_id": "00000000-0000-0000-0000-000000000001", "text": "",
        "content": [{"type":"image","mime_type":"image/png","data":"a".repeat(2 * 1024 * 1024 + 1024)}]
    });
    let response = fixture.request("sendManagedPromptContent", oversized_image)
        .header(RUNTIME_GENERATION_HEADER, "managed-test").send().await?;
    // The route accepts an envelope larger than 2 MiB, then core rejects the
    // invalid image before starting a turn. Other JSON routes retain 2 MiB.
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let response = fixture.request("sendManagedPrompt", json!({
        "session_id":"00000000-0000-0000-0000-000000000001", "text":"x".repeat(2 * 1024 * 1024)
    })).header(RUNTIME_GENERATION_HEADER, "managed-test").send().await?;
    assert_eq!(response.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    let response = fixture.request("sendManagedPromptContent", json!({
        "session_id":"00000000-0000-0000-0000-000000000001", "text":"x".repeat(5 * 1024 * 1024), "content":[]
    })).header(RUNTIME_GENERATION_HEADER, "managed-test").send().await?;
    assert_eq!(response.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    fixture.shutdown().await
}

#[tokio::test]
async fn uncertain_delivery_decisions_are_scoped_and_return_the_updated_snapshot()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let config = fixture.facade.save_agent_config(config_input()).await?;
    let session = fixture
        .facade
        .create_managed_session(CreateManagedSessionInput {
            agent_config_id: config.id,
            cwd: fixture.directory.path().to_string_lossy().into_owned(),
            title: "Delivery decision".into(),
        })
        .await?;
    let scope = ManagedFeedbackScope::from_session(&session.session)?;
    for (action, expected) in [("retry", "pending"), ("acknowledge", "delivered")] {
        let request = fixture
            .feedback
            .request_managed_feedback(
                &scope,
                RequestFeedbackInput {
                    request_id: None,
                    host_id: None,
                    host_session_id: String::new(),
                    title: Some("Review".into()),
                    what_happened: "Review delivery".into(),
                    actions: vec![ActionInput {
                        id: "review".into(),
                        instruction: "Review fixture".into(),
                    }],
                    context_refs: vec![],
                    attachments: vec![],
                    source_hint: None,
                    allow_finish: false,
                    final_summary: None,
                },
            )
            .await?;
        fixture
            .feedback
            .cancel_feedback(CancelFeedbackInput {
                request_id: request.request_id.clone(),
                reason: "Fixture cancellation".into(),
            })
            .await?;
        let now = "2026-09-04T12:00:00Z";
        fixture
            .store
            .claim_delivery(&request.request_id, "attempt", now)
            .await?
            .expect("pending delivery");
        fixture
            .store
            .finish_delivery(
                &request.request_id,
                "attempt",
                FeedbackDeliveryState::Uncertain,
                Some("fixture interruption"),
                now,
            )
            .await?;
        let resolved = fixture.call("resolveFeedbackDelivery", json!({"session_id":session.session.session_id,"request_id":request.request_id,"action":action})).await?;
        let delivery = resolved["deliveries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|delivery| delivery["request_id"] == request.request_id)
            .unwrap();
        assert_eq!(delivery["state"], expected);
        assert_eq!(delivery["session_id"], session.session.session_id);
    }
    fixture.shutdown().await
}
