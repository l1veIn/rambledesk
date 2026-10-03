use super::*;
use std::time::Duration;

struct DeliveryReadGate {
    store: Arc<SqliteFeedbackStore>,
    fail: AtomicBool,
    previous_batch: Mutex<Vec<String>>,
    completed_scan: tokio::sync::watch::Sender<Vec<String>>,
    finished: tokio::sync::watch::Sender<BTreeMap<String, FeedbackDelivery>>,
    claims: Mutex<BTreeMap<String, usize>>,
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
        // Entering the next pass proves the worker finished checking the previous
        // batch, including connection admission; observing its read alone does not.
        self.completed_scan
            .send_replace(std::mem::take(&mut *self.previous_batch.lock().unwrap()));
        let pending = self.store.list_pending_deliveries().await?;
        *self.previous_batch.lock().unwrap() = pending
            .iter()
            .map(|delivery| delivery.request_id.clone())
            .collect();
        Ok(pending)
    }
    async fn claim_delivery(
        &self,
        request_id: &str,
        attempt_id: &str,
        now: &str,
    ) -> Result<Option<FeedbackDelivery>, SessionRepositoryError> {
        let claimed = self
            .store
            .claim_delivery(request_id, attempt_id, now)
            .await?;
        if claimed.is_some() {
            *self
                .claims
                .lock()
                .unwrap()
                .entry(request_id.into())
                .or_default() += 1;
        }
        Ok(claimed)
    }
    async fn finish_delivery(
        &self,
        request_id: &str,
        attempt_id: &str,
        state: FeedbackDeliveryState,
        last_error: Option<&str>,
        now: &str,
    ) -> Result<FeedbackDelivery, SessionRepositoryError> {
        let finished = self
            .store
            .finish_delivery(request_id, attempt_id, state, last_error, now)
            .await?;
        if finished.state == FeedbackDeliveryState::Delivered {
            // The same attempt can finish idempotently before and after EndTurn.
            self.finished.send_modify(|deliveries| {
                deliveries.insert(request_id.into(), finished.clone());
            });
        }
        Ok(finished)
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

async fn await_persistence<T>(
    phase: &str,
    future: impl std::future::Future<Output = T>,
    app: &SessionApplication,
    session: &ManagedSessionSnapshot,
    driver: &FakeDriver,
) -> T {
    // This functional test spans multiple durable writes. SQLite's own busy
    // timeout is five seconds; it is not a five-second end-to-end latency budget.
    match tokio::time::timeout(Duration::from_secs(10), future).await {
        Ok(value) => value,
        Err(_) => {
            let connections = driver
                .connections
                .lock()
                .unwrap()
                .iter()
                .map(|connection| {
                    (
                        connection.closed.load(Ordering::SeqCst),
                        connection.stops.load(Ordering::SeqCst),
                        connection.prompts.load(Ordering::SeqCst),
                    )
                })
                .collect::<Vec<_>>();
            let state = tokio::time::timeout(Duration::from_secs(2), async {
                app.get_session(target(session))
                    .await
                    .map(|snapshot| (snapshot.runtime, snapshot.deliveries))
            })
            .await;
            panic!(
                "{phase} timed out; starts={}; connections(closed, stops, prompts)={connections:?}; runtime/queue={state:?}",
                driver.start_attempts.load(Ordering::SeqCst),
            );
        }
    }
}

#[tokio::test]
async fn stop_cleans_up_before_reporting_a_queue_read_failure_and_does_not_guess_admission() {
    let (dir, store, driver, app, config) = setup().await;
    let (completed_scan, mut scanned) = tokio::sync::watch::channel(Vec::new());
    let (finished, mut delivered) = tokio::sync::watch::channel(BTreeMap::new());
    let gate = Arc::new(DeliveryReadGate {
        store: store.clone(),
        fail: AtomicBool::new(false),
        previous_batch: Mutex::new(Vec::new()),
        completed_scan,
        finished,
        claims: Mutex::new(BTreeMap::new()),
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
    await_persistence(
        "worker scan while admission is unknown",
        scanned.wait_for(|batch| batch.contains(&old) && batch.contains(&new)),
        &app,
        &session,
        &driver,
    )
    .await
    .unwrap();
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 1);
    assert!(gate.claims.lock().unwrap().is_empty());
    let suppressed = app.get_session(target(&session)).await.unwrap();
    assert_eq!(
        suppressed.runtime.connection,
        SessionConnectionState::Stopped
    );
    assert_eq!(suppressed.runtime.last_error, None);
    assert_eq!(suppressed.deliveries.len(), 2);
    assert!(
        suppressed
            .deliveries
            .iter()
            .all(|delivery| delivery.state == FeedbackDeliveryState::Pending
                && delivery.attempt_id.is_none())
    );
    app.start_session(target(&session)).await.unwrap();
    await_persistence(
        "both feedback deliveries persisted",
        delivered
            .wait_for(|deliveries| deliveries.contains_key(&old) && deliveries.contains_key(&new)),
        &app,
        &session,
        &driver,
    )
    .await
    .unwrap();
    let persisted = store
        .list_session_deliveries(&session.session.session_id)
        .await
        .unwrap();
    assert_eq!(persisted.len(), 2);
    for request_id in [&old, &new] {
        let delivery = persisted
            .iter()
            .find(|delivery| &delivery.request_id == request_id)
            .unwrap();
        assert_eq!(delivery.state, FeedbackDeliveryState::Delivered);
        assert!(delivery.attempt_id.is_some());
        assert_eq!(gate.claims.lock().unwrap().get(request_id), Some(&1));
    }
    let resumed = driver.connections.lock().unwrap()[1].clone();
    await_persistence(
        "both continuation prompts reached the driver",
        async {
            while resumed.prompts.load(Ordering::SeqCst) < 2 {
                tokio::task::yield_now().await;
            }
        },
        &app,
        &session,
        &driver,
    )
    .await;
    assert_eq!(connection.prompts.load(Ordering::SeqCst), 0);
    assert_eq!(resumed.prompts.load(Ordering::SeqCst), 2);
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    app.shutdown().await.unwrap();
    store.close().await;
}
