use async_trait::async_trait;
use rambledesk_core::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::Notify;

#[tokio::test]
async fn initial_snapshot_is_bounded_and_older_pages_recover_the_complete_history() {
    let (_dir, store, app, _driver, ids) = setup().await;
    let id = &ids[0];
    for sequence in 1..=205 {
        store
            .append_activity(NewSessionActivity {
                id: format!("row-{sequence}"),
                session_id: id.clone(),
                turn_id: None,
                kind: SessionActivityKind::AgentMessage,
                text: format!("message {sequence}"),
                tool_call_id: None,
                created_at: "2026-09-05T12:00:00Z".into(),
                content: None,
            })
            .await
            .unwrap();
    }
    let recent = app
        .get_session(ManagedSessionInput {
            session_id: id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(recent.activities.len(), 100);
    assert_eq!(recent.activities.first().unwrap().sequence, 106);
    assert_eq!(recent.activities.last().unwrap().sequence, 205);
    let older = app
        .list_activity_history(ListManagedSessionActivityInput {
            session_id: id.clone(),
            before_sequence: 106,
            limit: None,
        })
        .await
        .unwrap();
    assert!(older.has_more);
    assert_eq!(older.activities.len(), 100);
    assert_eq!(older.activities.first().unwrap().sequence, 6);
    assert_eq!(older.activities.last().unwrap().sequence, 105);
    let first = app
        .list_activity_history(ListManagedSessionActivityInput {
            session_id: id.clone(),
            before_sequence: 6,
            limit: None,
        })
        .await
        .unwrap();
    assert!(!first.has_more);
    let all = first
        .activities
        .iter()
        .chain(&older.activities)
        .chain(&recent.activities)
        .map(|row| row.sequence)
        .collect::<Vec<_>>();
    assert_eq!(all, (1..=205).collect::<Vec<_>>());
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn context_usage_is_instance_scoped_and_unknown_after_restart() {
    let (_dir, store, app, driver, ids) = setup().await;
    let id = &ids[0];
    let old = driver.connections.lock().unwrap()[id].clone();
    let input = ManagedSessionInput {
        session_id: id.clone(),
    };
    assert_eq!(
        app.get_session(input.clone())
            .await
            .unwrap()
            .runtime
            .context_usage,
        None
    );
    old.observer
        .observe(AgentSessionEvent::ContextUsage(SessionContextUsage {
            used: 40000,
            size: 128000,
        }))
        .await
        .unwrap();
    let snapshot = app.get_session(input.clone()).await.unwrap();
    assert_eq!(
        snapshot.runtime.context_usage,
        Some(SessionContextUsage {
            used: 40000,
            size: 128000
        })
    );
    assert!(snapshot.activities.is_empty());
    assert_eq!(
        app.get_session(ManagedSessionInput {
            session_id: ids[1].clone()
        })
        .await
        .unwrap()
        .runtime
        .context_usage,
        None
    );
    app.stop_session(input.clone()).await.unwrap();
    app.start_session(input.clone()).await.unwrap();
    assert_eq!(
        app.get_session(input.clone())
            .await
            .unwrap()
            .runtime
            .context_usage,
        None
    );
    old.observer
        .observe(AgentSessionEvent::ContextUsage(SessionContextUsage {
            used: 999,
            size: 1000,
        }))
        .await
        .unwrap();
    assert_eq!(
        app.get_session(input.clone())
            .await
            .unwrap()
            .runtime
            .context_usage,
        None
    );
    let current = driver.connections.lock().unwrap()[id].clone();
    current
        .observer
        .observe(AgentSessionEvent::ContextUsage(SessionContextUsage {
            used: 100,
            size: 128000,
        }))
        .await
        .unwrap();
    app.shutdown().await.unwrap();
    let fresh = SessionApplication::new(store.clone(), store.clone(), driver);
    assert_eq!(
        fresh
            .get_session(input)
            .await
            .unwrap()
            .runtime
            .context_usage,
        None
    );
    fresh.shutdown().await.unwrap();
    store.close().await;
}

#[derive(Default)]
struct Driver {
    connections: Mutex<HashMap<String, Arc<Connection>>>,
}
struct Connection {
    observer: Arc<dyn AgentSessionObserver>,
    finish: Notify,
    closed: AtomicBool,
}
#[async_trait]
impl AgentSessionConnection for Connection {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
    async fn prompt(&self, _: &str) -> Result<String, AgentDriverError> {
        self.finish.notified().await;
        Ok("EndTurn".into())
    }
    async fn cancel(&self) -> Result<(), AgentDriverError> {
        self.finish.notify_one();
        Ok(())
    }
    async fn respond_permission(&self, _: &str, _: Option<&str>) -> Result<(), AgentDriverError> {
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
        let connection = Arc::new(Connection {
            observer: launch.observer,
            finish: Notify::new(),
            closed: AtomicBool::new(false),
        });
        self.connections
            .lock()
            .unwrap()
            .insert(launch.session.session_id.clone(), connection.clone());
        Ok(StartedAgentSession {
            connection,
            remote_session_id: format!("remote-{}", launch.session.session_id),
            capabilities: AgentSessionCapabilities::default(),
        })
    }
    async fn check(&self, _: &AgentConfig) -> Result<AgentSessionCapabilities, AgentDriverError> {
        Ok(AgentSessionCapabilities::default())
    }
}

async fn setup() -> (
    tempfile::TempDir,
    Arc<SqliteFeedbackStore>,
    SessionApplication,
    Arc<Driver>,
    Vec<String>,
) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
            .await
            .unwrap(),
    );
    let driver = Arc::new(Driver::default());
    let app = SessionApplication::new(store.clone(), store.clone(), driver.clone());
    let config = app
        .save_agent_config(SaveAgentConfigInput {
            catalog_id: None,
            id: None,
            name: "Fixture".into(),
            host_id: "fixture".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "fixture".into(),
            args: vec![],
            env: BTreeMap::new(),
        })
        .await
        .unwrap();
    let mut ids = vec![];
    for title in ["one", "two"] {
        ids.push(
            app.create_session(CreateManagedSessionInput {
                agent_config_id: config.id.clone(),
                cwd: dir.path().to_string_lossy().into(),
                title: title.into(),
            })
            .await
            .unwrap()
            .session
            .session_id,
        );
    }
    (dir, store, app, driver, ids)
}
async fn prompt(app: &SessionApplication, id: &str) {
    app.send_prompt(SendManagedPromptInput {
        session_id: id.into(),
        text: "Run".into(),
    })
    .await
    .unwrap();
}
async fn idle(app: &SessionApplication, id: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if app
                .get_session(ManagedSessionInput {
                    session_id: id.into(),
                })
                .await
                .unwrap()
                .runtime
                .activity
                == SessionActivityState::Idle
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
fn message(text: &str, kind: SessionActivityKind) -> AgentSessionEvent {
    AgentSessionEvent::MessageChunk {
        kind,
        block: SessionContentBlock::Text { text: text.into() },
        truncated: false,
    }
}
fn tool(patch: SessionToolCallPatch) -> AgentSessionEvent {
    AgentSessionEvent::ToolCall {
        tool_call_id: "same-tool".into(),
        patch,
    }
}

#[tokio::test]
async fn interleaved_tool_patches_preserve_order_turn_and_session_isolation_after_reload() {
    let (dir, store, app, driver, ids) = setup().await;
    prompt(&app, &ids[0]).await;
    prompt(&app, &ids[1]).await;
    let one = driver.connections.lock().unwrap()[&ids[0]].clone();
    let two = driver.connections.lock().unwrap()[&ids[1]].clone();
    one.observer
        .observe(message("Before ", SessionActivityKind::AgentMessage))
        .await
        .unwrap();
    one.observer
        .observe(message("tool", SessionActivityKind::AgentMessage))
        .await
        .unwrap();
    // Update-before-initial still creates exactly one anchor for this tool.
    one.observer
        .observe(tool(SessionToolCallPatch {
            status: Some(SessionToolStatus::InProgress),
            ..Default::default()
        }))
        .await
        .unwrap();
    one.observer
        .observe(tool(SessionToolCallPatch {
            title: Some("Read file".into()),
            raw_input: Some(r#"{"path":"one"}"#.into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    one.observer
        .observe(message("After ", SessionActivityKind::AgentMessage))
        .await
        .unwrap();
    one.observer
        .observe(tool(SessionToolCallPatch {
            status: Some(SessionToolStatus::Completed),
            raw_output: Some("result".into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    one.observer
        .observe(message("tool", SessionActivityKind::AgentMessage))
        .await
        .unwrap();
    one.observer
        .observe(message("Think", SessionActivityKind::AgentThought))
        .await
        .unwrap();
    one.observer
        .observe(message("Done", SessionActivityKind::AgentMessage))
        .await
        .unwrap();
    two.observer
        .observe(tool(SessionToolCallPatch {
            title: Some("Other session".into()),
            ..Default::default()
        }))
        .await
        .unwrap();
    one.finish.notify_one();
    two.finish.notify_one();
    idle(&app, &ids[0]).await;
    idle(&app, &ids[1]).await;
    let first = store
        .list_session_activity(&ids[0], None, 100)
        .await
        .unwrap();
    let rich = first
        .iter()
        .filter(|row| row.content.is_some())
        .collect::<Vec<_>>();
    assert_eq!(rich.len(), 5);
    assert_eq!(rich[0].text, "Before tool");
    assert_eq!(rich[2].text, "After tool");
    assert_eq!(rich[3].kind, SessionActivityKind::AgentThought);
    let Some(SessionActivityContent::ToolCall { tool }) = &rich[1].content else {
        panic!("tool")
    };
    assert_eq!(tool.title, "Read file");
    assert_eq!(tool.status, SessionToolStatus::Completed);
    assert_eq!(tool.raw_input.as_deref(), Some(r#"{"path":"one"}"#));
    assert_eq!(tool.raw_output.as_deref(), Some("result"));
    let first_turn = rich[1].turn_id.clone();
    // Ended-turn traffic is ignored, as are load replays outside an active turn.
    one.observer
        .observe(message("late duplicate", SessionActivityKind::AgentMessage))
        .await
        .unwrap();
    assert_eq!(
        store
            .list_session_activity(&ids[0], None, 100)
            .await
            .unwrap(),
        first
    );
    prompt(&app, &ids[0]).await;
    one.observer
        .observe(tool_event_for_next_turn())
        .await
        .unwrap();
    app.cancel_prompt(ManagedSessionInput {
        session_id: ids[0].clone(),
    })
    .await
    .unwrap();
    idle(&app, &ids[0]).await;
    let before_reopen = store
        .list_session_activity(&ids[0], None, 100)
        .await
        .unwrap();
    let tools = before_reopen
        .iter()
        .filter(|row| row.kind == SessionActivityKind::ToolCall)
        .collect::<Vec<_>>();
    assert_eq!(tools.len(), 2);
    assert_ne!(tools[1].turn_id, first_turn);
    let other = store
        .list_session_activity(&ids[1], None, 100)
        .await
        .unwrap();
    assert!(
        other
            .iter()
            .any(|row| row.text.starts_with("Other session"))
    );
    assert!(!other.iter().any(|row| row.text.starts_with("Read file")));
    app.shutdown().await.unwrap();
    store.close().await;
    let reopened = SqliteFeedbackStore::connect(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        reopened
            .list_session_activity(&ids[0], None, 100)
            .await
            .unwrap(),
        before_reopen
    );
    assert_eq!(
        reopened
            .list_session_activity(&ids[1], None, 100)
            .await
            .unwrap(),
        other
    );
    reopened.close().await;
}

fn tool_event_for_next_turn() -> AgentSessionEvent {
    tool(SessionToolCallPatch {
        title: Some("Next turn".into()),
        ..Default::default()
    })
}

#[tokio::test]
async fn long_multibyte_stream_is_bounded_without_failing_the_turn() {
    let (_dir, store, app, driver, ids) = setup().await;
    prompt(&app, &ids[0]).await;
    let connection = driver.connections.lock().unwrap()[&ids[0]].clone();
    for _ in 0..3 {
        connection
            .observer
            .observe(message(
                &"字".repeat(50_000),
                SessionActivityKind::AgentMessage,
            ))
            .await
            .unwrap();
    }
    connection.finish.notify_one();
    idle(&app, &ids[0]).await;
    let rows = store
        .list_session_activity(&ids[0], None, 100)
        .await
        .unwrap();
    let rich = rows
        .iter()
        .filter(|row| row.kind == SessionActivityKind::AgentMessage)
        .collect::<Vec<_>>();
    assert_eq!(rich.len(), 1);
    assert!(rich[0].text.len() <= MAX_ACTIVITY_TEXT_BYTES);
    assert!(matches!(
        rich[0].content,
        Some(SessionActivityContent::Message {
            truncated: true,
            ..
        })
    ));
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn omitted_media_preserves_metadata_and_later_text_in_order() {
    let (_dir, store, app, driver, ids) = setup().await;
    prompt(&app, &ids[0]).await;
    let connection = driver.connections.lock().unwrap()[&ids[0]].clone();
    connection
        .observer
        .observe(AgentSessionEvent::MessageChunk {
            kind: SessionActivityKind::AgentMessage,
            block: SessionContentBlock::Image {
                mime_type: "image/png".into(),
                data: None,
                uri: None,
            },
            truncated: true,
        })
        .await
        .unwrap();
    connection
        .observer
        .observe(message(
            "Image explanation",
            SessionActivityKind::AgentMessage,
        ))
        .await
        .unwrap();
    connection.finish.notify_one();
    idle(&app, &ids[0]).await;
    let rows = store
        .list_session_activity(&ids[0], None, 100)
        .await
        .unwrap();
    let row = rows
        .iter()
        .find(|row| row.kind == SessionActivityKind::AgentMessage)
        .unwrap();
    let Some(SessionActivityContent::Message { blocks, truncated }) = &row.content else {
        panic!("message")
    };
    assert!(*truncated);
    assert_eq!(blocks.len(), 2);
    assert!(matches!(
        blocks[0],
        SessionContentBlock::Image { data: None, .. }
    ));
    assert_eq!(
        blocks[1],
        SessionContentBlock::Text {
            text: "Image explanation".into()
        }
    );
    app.shutdown().await.unwrap();
    store.close().await;
}
