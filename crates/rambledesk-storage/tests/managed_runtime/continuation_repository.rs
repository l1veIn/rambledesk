use super::*;
use std::time::Duration;

struct DeliveryReadGate {
    store: Arc<SqliteFeedbackStore>,
    fail: AtomicBool,
}

#[async_trait]
impl FeedbackDeliveryRepository for DeliveryReadGate {
    async fn list_session_deliveries(
        &self,
        session_id: &str,
    ) -> Result<Vec<FeedbackDelivery>, SessionRepositoryError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(SessionRepositoryError::CorruptData);
        }
        self.store.list_session_deliveries(session_id).await
    }
    async fn list_pending_deliveries(
        &self,
    ) -> Result<Vec<FeedbackDelivery>, SessionRepositoryError> {
        self.store.list_pending_deliveries().await
    }
    async fn claim_delivery(
        &self,
        request_id: &str,
        attempt_id: &str,
        now: &str,
    ) -> Result<Option<FeedbackDelivery>, SessionRepositoryError> {
        self.store.claim_delivery(request_id, attempt_id, now).await
    }
    async fn finish_delivery(
        &self,
        request_id: &str,
        attempt_id: &str,
        state: FeedbackDeliveryState,
        last_error: Option<&str>,
        now: &str,
    ) -> Result<FeedbackDelivery, SessionRepositoryError> {
        self.store
            .finish_delivery(request_id, attempt_id, state, last_error, now)
            .await
    }
    async fn recover_interrupted_deliveries(
        &self,
        now: &str,
    ) -> Result<u64, SessionRepositoryError> {
        self.store.recover_interrupted_deliveries(now).await
    }
    async fn discard_session_deliveries(
        &self,
        session_id: &str,
        now: &str,
    ) -> Result<u64, SessionRepositoryError> {
        self.store.discard_session_deliveries(session_id, now).await
    }
    async fn resolve_delivery(
        &self,
        request_id: &str,
        session_id: &str,
        action: ResolveDeliveryAction,
        now: &str,
    ) -> Result<FeedbackDelivery, SessionRepositoryError> {
        self.store
            .resolve_delivery(request_id, session_id, action, now)
            .await
    }
}

#[tokio::test]
async fn stop_cleans_up_before_reporting_a_queue_read_failure_and_does_not_guess_admission() {
    let (dir, store, driver, app, config) = setup().await;
    let gate = Arc::new(DeliveryReadGate {
        store: store.clone(),
        fail: AtomicBool::new(false),
    });
    let app = app.with_deliveries(gate.clone());
    let session = app
        .create_session(input(&dir, &config, "Unavailable queue"))
        .await
        .unwrap();
    let old = continuation_startup::submit_review(&store, &session).await;
    let connection = driver.connections.lock().unwrap()[0].clone();
    gate.fail.store(true, Ordering::SeqCst);
    assert!(matches!(
        app.stop_session(target(&session)).await,
        Err(SessionError::Repository(
            SessionRepositoryError::CorruptData
        ))
    ));
    assert!(connection.closed.load(Ordering::SeqCst));
    assert_eq!(connection.stops.load(Ordering::SeqCst), 1);
    gate.fail.store(false, Ordering::SeqCst);
    let stopped = app.get_session(target(&session)).await.unwrap();
    assert_eq!(stopped.runtime.connection, SessionConnectionState::Stopped);
    assert_eq!(stopped.runtime.last_error, None);
    app.start_delivery_worker().await.unwrap();
    let new = continuation_startup::submit_review(&store, &session).await;
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 1);
    app.start_session(target(&session)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !continuation_startup::delivered(&store, &session.session.session_id, &old).await
            || !continuation_startup::delivered(&store, &session.session.session_id, &new).await
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    app.shutdown().await.unwrap();
    store.close().await;
}
