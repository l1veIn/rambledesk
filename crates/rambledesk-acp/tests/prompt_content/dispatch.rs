use super::*;
use async_trait::async_trait;
use rambledesk_acp::AcpSessionDriver;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

#[derive(Default)]
struct DispatchGate {
    held: AtomicBool,
    entered: Notify,
    release: Notify,
}

struct GatedDriver(Arc<DispatchGate>);

#[async_trait]
impl AgentSessionDriver for GatedDriver {
    async fn start(
        &self,
        launch: AgentSessionLaunch,
    ) -> Result<StartedAgentSession, AgentDriverError> {
        let mut started = AcpSessionDriver.start(launch).await?;
        started.connection = Arc::new(GatedConnection {
            inner: started.connection,
            gate: self.0.clone(),
        });
        Ok(started)
    }

    async fn check(
        &self,
        config: &AgentConfig,
    ) -> Result<AgentSessionCapabilities, AgentDriverError> {
        AcpSessionDriver.check(config).await
    }
}

struct GatedConnection {
    inner: Arc<dyn AgentSessionConnection>,
    gate: Arc<DispatchGate>,
}

#[async_trait]
impl AgentSessionConnection for GatedConnection {
    fn prepare_prompt(&self) {
        self.inner.prepare_prompt();
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    async fn cancel(&self) -> Result<(), AgentDriverError> {
        self.inner.cancel().await
    }

    async fn respond_interaction(
        &self,
        id: &str,
        response: SessionInteractionResponse,
    ) -> Result<(), AgentDriverError> {
        self.inner.respond_interaction(id, response).await
    }

    async fn prompt(&self, text: &str) -> Result<String, AgentDriverError> {
        self.inner.prompt(text).await
    }

    async fn prompt_content(
        &self,
        blocks: &[SessionPromptContent],
    ) -> Result<String, AgentDriverError> {
        if !self.gate.held.swap(true, Ordering::SeqCst) {
            self.gate.entered.notify_one();
            self.gate.release.notified().await;
        }
        self.inner.prompt_content(blocks).await
    }

    async fn stop(&self) -> Result<(), AgentDriverError> {
        self.inner.stop().await
    }
}

#[tokio::test]
async fn cancellation_before_protocol_dispatch_is_preserved_and_next_turn_is_independent() {
    let (dir, store, _unused_app, config) = setup_fixture("prompt_content", "full").await;
    let gate = Arc::new(DispatchGate::default());
    let app = SessionApplication::new(
        store.clone(),
        store.clone(),
        Arc::new(GatedDriver(gate.clone())),
    );
    let session = create(&app, &dir, &config, "Gated cancellation").await;
    let id = id(&session);
    app.send_prompt_content(input(&id, "wait", vec![image()]))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), gate.entered.notified())
        .await
        .unwrap();
    // The real ACP driver has not been polled for this prompt yet. Cancellation
    // must remain latched across this handoff, not be reset by late delivery.
    app.cancel_prompt(id.clone()).await.unwrap();
    gate.release.notify_one();
    let result = tokio::time::timeout(std::time::Duration::from_secs(3), idle(&app, &id)).await;
    if result.is_err() {
        app.stop_session(id.clone()).await.unwrap();
        app.shutdown().await.unwrap();
        store.close().await;
        panic!("cancellation was lost before the protocol prompt was dispatched");
    }
    let cancelled = result.unwrap();
    assert_eq!(
        cancelled.runtime.connection,
        SessionConnectionState::Connected
    );
    assert_eq!(cancelled.runtime.instance_id, session.runtime.instance_id);
    assert_eq!(
        cancelled
            .activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::UserMessage)
            .count(),
        1
    );
    assert!(
        cancelled
            .activities
            .iter()
            .any(|row| row.text.contains("Cancelled"))
    );
    app.send_prompt(SendManagedPromptInput {
        session_id: id.session_id.clone(),
        text: "Legacy after early cancellation".into(),
    })
    .await
    .unwrap();
    let next = idle(&app, &id).await;
    assert_eq!(next.runtime.connection, SessionConnectionState::Connected);
    assert_eq!(next.runtime.instance_id, session.runtime.instance_id);
    let users = next
        .activities
        .iter()
        .filter(|row| row.kind == SessionActivityKind::UserMessage)
        .collect::<Vec<_>>();
    assert_eq!(users.len(), 2);
    assert!(users[0].content.is_some());
    assert_eq!(users[1].content, None);
    assert_eq!(users[1].text, "Legacy after early cancellation");
    assert!(
        next.activities
            .iter()
            .any(|row| row.kind == SessionActivityKind::AgentMessage
                && row.text.contains("Legacy after early cancellation"))
    );
    app.shutdown().await.unwrap();
    store.close().await;
}
