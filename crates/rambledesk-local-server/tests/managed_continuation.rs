use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_local_server::{
    AccessToken, LocalManagedFeedbackProvider, ServerConfig, ServerHandle,
    start_server_with_managed,
};
use rambledesk_storage::SqliteFeedbackStore;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

struct Fixture {
    dir: tempfile::TempDir,
    store: Arc<SqliteFeedbackStore>,
    feedback: FeedbackApplication,
    app: SessionApplication,
    server: ServerHandle,
    config: String,
}
impl Fixture {
    async fn new(mode: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
                .await
                .unwrap(),
        );
        let feedback = (*store).clone().into_application();
        let provider = Arc::new(LocalManagedFeedbackProvider::new(feedback.clone()));
        let server = start_server_with_managed(
            ServerConfig::new(AccessToken::generate()).with_port(0),
            feedback.clone(),
            provider.clone(),
        )
        .await
        .unwrap();
        let app = SessionApplication::new(store.clone(), store.clone(), Arc::new(AcpSessionDriver))
            .with_feedback_provider(provider)
            .with_deliveries(store.clone())
            .with_deletions(store.clone());
        app.start_delivery_worker().await.unwrap();
        let config = app
            .save_agent_config(SaveAgentConfigInput {
                catalog_id: None,
                id: None,
                name: "Fixture".into(),
                host_id: "fixture".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: "node".into(),
                args: vec![
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/managed_agent.mjs")
                        .to_string_lossy()
                        .into_owned(),
                    mode.into(),
                ],
                env: BTreeMap::from([(
                    "FIXTURE_DIAGNOSTIC".into(),
                    dir.path().join("diagnostic").to_string_lossy().into_owned(),
                )]),
            })
            .await
            .unwrap()
            .id;
        Self {
            dir,
            store,
            feedback,
            app,
            server,
            config,
        }
    }
    async fn create(&self, title: &str) -> String {
        let snapshot = self
            .app
            .create_session(CreateManagedSessionInput {
                agent_config_id: self.config.clone(),
                cwd: self.dir.path().to_string_lossy().into_owned(),
                title: title.into(),
            })
            .await
            .unwrap();
        assert_eq!(
            snapshot.runtime.connection,
            SessionConnectionState::Connected,
            "{:?} {:?}",
            snapshot.runtime,
            std::fs::read_to_string(self.dir.path().join("diagnostic"))
        );
        snapshot.session.session_id
    }
    async fn snapshot(&self, id: &str) -> ManagedSessionSnapshot {
        self.app
            .get_session(ManagedSessionInput {
                session_id: id.into(),
            })
            .await
            .unwrap()
    }
    async fn request(&self, session: &str, wait: bool) -> String {
        let previous = self
            .snapshot(session)
            .await
            .activities
            .last()
            .map(|row| row.sequence)
            .unwrap_or(0);
        self.app
            .send_prompt(SendManagedPromptInput {
                session_id: session.into(),
                text: if wait { "request_wait" } else { "request" }.into(),
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let snapshot = self.snapshot(session).await;
                if let Some(row) = snapshot
                    .activities
                    .iter()
                    .rev()
                    .find(|row| row.sequence > previous && row.text.starts_with("REQUEST "))
                {
                    return row.text[8..].to_owned();
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|error| {
            panic!(
                "{error}: {:?}",
                std::fs::read_to_string(self.dir.path().join("diagnostic"))
            )
        })
    }
    async fn submitted(&self, request: &str) {
        let revision = self
            .feedback
            .save_feedback_draft(SaveDraftInput {
                request_id: request.into(),
                expected_revision: 0,
                document_json: r#"{"schemaVersion":2,"doc":{"type":"doc"}}"#.into(),
                body_markdown: "Please continue with the reviewed changes.".into(),
            })
            .await
            .unwrap()
            .saved_revision;
        self.feedback
            .submit_feedback(SubmitFeedbackInput {
                request_id: request.into(),
                expected_revision: revision,
                cooked_markdown: None,
                cooking_model: None,
                uncooked_markdown: None,
            })
            .await
            .unwrap();
    }
    async fn delivered(
        &self,
        session: &str,
        state: FeedbackDeliveryState,
    ) -> ManagedSessionSnapshot {
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let snapshot = self.snapshot(session).await;
                if snapshot.deliveries.iter().any(|item| item.state == state) {
                    return snapshot;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn close(self) {
        self.app.shutdown().await.unwrap();
        self.server.shutdown().await.unwrap();
        self.store.close().await;
    }
}

#[tokio::test]
async fn scoped_mcp_feedback_waits_for_idle_then_continues_the_original_session_once() {
    let fixture = Fixture::new("normal").await;
    let first = fixture.create("One").await;
    let second = fixture.create("Two").await;
    let request = fixture.request(&first, true).await;
    fixture.submitted(&request).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        fixture.snapshot(&first).await.deliveries[0].state,
        FeedbackDeliveryState::Pending
    );
    assert!(fixture.snapshot(&second).await.activities.is_empty());
    fixture
        .app
        .cancel_prompt(ManagedSessionInput {
            session_id: first.clone(),
        })
        .await
        .unwrap();
    let done = fixture
        .delivered(&first, FeedbackDeliveryState::Delivered)
        .await;
    assert!(
        done.activities
            .iter()
            .any(|item| item.text == format!("CONTINUED {request} feedback_submitted"))
    );
    assert!(
        matches!(done.session.management,SessionManagement::Managed{remote_session_id:Some(ref id),..} if id=="original")
    );
    fixture
        .feedback
        .submit_feedback(SubmitFeedbackInput {
            request_id: request.clone(),
            expected_revision: 0,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        fixture
            .snapshot(&first)
            .await
            .activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::UserMessage
                && row.text.contains("human feedback is ready"))
            .count(),
        1
    );
    let next = fixture.request(&first, false).await;
    fixture
        .feedback
        .approve_feedback(ApproveFeedbackInput { request_id: next })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if fixture
                .snapshot(&first)
                .await
                .deliveries
                .iter()
                .filter(|item| item.state == FeedbackDeliveryState::Delivered)
                .count()
                == 2
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn disconnect_after_feedback_read_is_uncertain_and_never_blindly_replayed() {
    let fixture = Fixture::new("fail_continue").await;
    let session = fixture.create("One").await;
    let request = fixture.request(&session, false).await;
    fixture.submitted(&request).await;
    let uncertain = fixture
        .delivered(&session, FeedbackDeliveryState::Uncertain)
        .await;
    assert!(
        uncertain
            .activities
            .iter()
            .any(|row| row.text.starts_with("CONTINUED"))
    );
    fixture
        .app
        .start_session(ManagedSessionInput {
            session_id: session.clone(),
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        fixture.snapshot(&session).await.deliveries[0].state,
        FeedbackDeliveryState::Uncertain
    );
    fixture
        .app
        .resolve_feedback_delivery(ResolveFeedbackDeliveryInput {
            session_id: session.clone(),
            request_id: request,
            action: ResolveDeliveryAction::Acknowledge,
        })
        .await
        .unwrap();
    assert_eq!(
        fixture.snapshot(&session).await.deliveries[0].state,
        FeedbackDeliveryState::Delivered
    );
    fixture.close().await;
}

#[tokio::test]
async fn direct_delete_stops_a_busy_session_discards_feedback_and_keeps_its_neighbor() {
    let fixture = Fixture::new("normal").await;
    let first = fixture.create("Busy").await;
    let other = fixture.create("Neighbor").await;
    let request = fixture.request(&first, true).await;
    fixture.submitted(&request).await;
    let input = ManagedSessionInput {
        session_id: first.clone(),
    };
    fixture
        .app
        .delete_managed_session(input.clone())
        .await
        .unwrap();
    fixture.app.delete_managed_session(input).await.unwrap();
    assert!(matches!(
        fixture.store.get_session(&first).await,
        Err(SessionRepositoryError::SessionNotFound)
    ));
    assert!(fixture.store.get_request(&request).await.is_err());
    assert!(
        fixture
            .store
            .list_session_deliveries(&first)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture.snapshot(&other).await.runtime.connection,
        SessionConnectionState::Connected
    );
    assert!(fixture.snapshot(&other).await.activities.is_empty());
    // A zero-feedback session can also be removed directly.
    fixture
        .app
        .delete_managed_session(ManagedSessionInput { session_id: other })
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn durable_deletion_intent_blocks_new_work_and_can_be_finished_by_a_new_runtime() {
    let fixture = Fixture::new("normal").await;
    let session = fixture.create("Deleting").await;
    fixture
        .app
        .stop_session(ManagedSessionInput {
            session_id: session.clone(),
        })
        .await
        .unwrap();
    fixture
        .store
        .begin_managed_session_deletion(&session, "2026-09-04T00:00:00Z")
        .await
        .unwrap();
    let app = SessionApplication::new(
        fixture.store.clone(),
        fixture.store.clone(),
        Arc::new(AcpSessionDriver),
    )
    .with_deletions(fixture.store.clone());
    assert!(
        app.get_session(ManagedSessionInput {
            session_id: session.clone()
        })
        .await
        .unwrap()
        .deleting
    );
    assert!(matches!(
        app.start_session(ManagedSessionInput {
            session_id: session.clone()
        })
        .await,
        Err(SessionError::NotConnected)
    ));
    app.delete_managed_session(ManagedSessionInput {
        session_id: session,
    })
    .await
    .unwrap();
    app.shutdown().await.unwrap();
    fixture.close().await;
}
