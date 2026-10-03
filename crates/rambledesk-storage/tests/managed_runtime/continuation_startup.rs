use super::*;
use std::{future::Future, time::Duration};

async fn eventually<F, P>(mut predicate: P)
where
    F: Future<Output = bool>,
    P: FnMut() -> F,
{
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate().await {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("continuation did not reach its expected state");
}

pub(super) async fn submit_review(
    store: &SqliteFeedbackStore,
    session: &ManagedSessionSnapshot,
) -> String {
    let feedback = store.clone().into_application();
    let request = cancelled_feedback::request_review(&feedback, session).await;
    let workspace = feedback
        .get_feedback_workspace(request.request_id.clone())
        .await
        .unwrap();
    let draft = feedback
        .save_feedback_draft(SaveDraftInput {
            request_id: request.request_id.clone(),
            document_json: r#"{"type":"doc","content":[]}"#.into(),
            body_markdown: "Keep the work and continue with this feedback.".into(),
            expected_revision: workspace.draft.saved_revision,
        })
        .await
        .unwrap();
    let submitted = WorkbenchTerminalOperations::without_observer(feedback)
        .submit_feedback(SubmitFeedbackInput {
            request_id: request.request_id.clone(),
            expected_revision: draft.saved_revision,
            cooked_markdown: None,
            cooking_model: None,
            uncooked_markdown: None,
        })
        .await
        .unwrap();
    assert_eq!(submitted.status, FeedbackStatus::Completed);
    request.request_id
}

pub(super) async fn delivered(
    store: &SqliteFeedbackStore,
    session_id: &str,
    request_id: &str,
) -> bool {
    store
        .list_session_deliveries(session_id)
        .await
        .unwrap()
        .iter()
        .any(|delivery| {
            delivery.request_id == request_id && delivery.state == FeedbackDeliveryState::Delivered
        })
}

#[tokio::test]
async fn stopped_feedback_submission_resumes_without_an_agent_view_and_connected_submission_reuses_it()
 {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Review"))
        .await
        .unwrap();
    app.stop_session(target(&session)).await.unwrap();
    app.start_delivery_worker().await.unwrap();
    let request = submit_review(&store, &session).await;
    eventually(|| delivered(&store, &session.session.session_id, &request)).await;
    let resumed = app.get_session(target(&session)).await.unwrap();
    assert_eq!(resumed.session.management, session.session.management);
    assert_ne!(resumed.runtime.instance_id, session.runtime.instance_id);
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    let second = submit_review(&store, &session).await;
    eventually(|| delivered(&store, &session.session.session_id, &second)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(
        driver.connections.lock().unwrap()[1]
            .prompts
            .load(Ordering::SeqCst),
        2
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn a_fresh_runtime_owner_initializes_only_after_a_durable_feedback_submission() {
    let (dir, store, _, original, config) = setup().await;
    let original = original.with_recovery(store.clone());
    let session = original
        .create_session(input(&dir, &config, "Restore"))
        .await
        .unwrap();
    original.shutdown().await.unwrap();
    let driver = Arc::new(FakeDriver::default());
    let app = SessionApplication::new(store.clone(), store.clone(), driver.clone())
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    app.start_delivery_worker().await.unwrap();
    let status = app.get_feedback_status(target(&session)).await.unwrap();
    assert_eq!(status.connection, SessionConnectionState::Stopped);
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 0);
    let request = submit_review(&store, &session).await;
    eventually(|| delivered(&store, &session.session.session_id, &request)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        driver.starts.lock().unwrap()[0].1,
        Some(format!("remote-{}", session.session.session_id))
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn failed_continuation_initialization_is_visible_and_never_repeated_by_polling() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Failure"))
        .await
        .unwrap();
    app.stop_session(target(&session)).await.unwrap();
    driver.fail.store(true, Ordering::SeqCst);
    app.start_delivery_worker().await.unwrap();
    let request = submit_review(&store, &session).await;
    eventually(|| async {
        app.get_feedback_status(target(&session))
            .await
            .unwrap()
            .connection
            == SessionConnectionState::Failed
    })
    .await;
    let failed = app.get_session(target(&session)).await.unwrap();
    assert!(
        failed
            .runtime
            .last_error
            .as_deref()
            .unwrap()
            .contains("fixture launch failed")
    );
    assert_eq!(failed.deliveries[0].state, FeedbackDeliveryState::Pending);
    assert_eq!(failed.deliveries[0].attempt_id, None);
    assert_eq!(failed.deliveries[0].last_error, None);
    assert_eq!(
        failed.runtime.failure.as_ref().unwrap().stage,
        AgentFailureStage::Launch
    );
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    driver.fail.store(false, Ordering::SeqCst);
    app.start_session(target(&session)).await.unwrap();
    eventually(|| delivered(&store, &session.session.session_id, &request)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 3);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn stop_interrupts_pending_initialization_until_a_new_submission_admits_connection() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Stop"))
        .await
        .unwrap();
    app.stop_session(target(&session)).await.unwrap();
    driver.hang.store(true, Ordering::SeqCst);
    app.start_delivery_worker().await.unwrap();
    let first = submit_review(&store, &session).await;
    eventually(|| async { driver.starting_count.load(Ordering::SeqCst) == 1 }).await;
    app.stop_session(target(&session)).await.unwrap();
    driver.hang.store(false, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(
        app.get_feedback_status(target(&session))
            .await
            .unwrap()
            .connection,
        SessionConnectionState::Stopped
    );
    assert!(!delivered(&store, &session.session.session_id, &first).await);
    let second = submit_review(&store, &session).await;
    eventually(|| async {
        delivered(&store, &session.session.session_id, &first).await
            && delivered(&store, &session.session.session_id, &second).await
    })
    .await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 3);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn a_waiting_permission_is_neither_answered_nor_reconnected_for_feedback() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app.with_deliveries(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Permission"))
        .await
        .unwrap();
    let connection = driver.connections.lock().unwrap()[0].clone();
    connection.hold_prompt.store(true, Ordering::SeqCst);
    connection
        .permission_on_prompt
        .store(true, Ordering::SeqCst);
    app.send_prompt(SendManagedPromptInput {
        session_id: session.session.session_id.clone(),
        text: "Wait for permission".into(),
    })
    .await
    .unwrap();
    eventually(|| async {
        app.get_feedback_status(target(&session))
            .await
            .unwrap()
            .activity
            == SessionActivityState::WaitingInput
    })
    .await;
    app.start_delivery_worker().await.unwrap();
    let request = submit_review(&store, &session).await;
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(connection.prompts.load(Ordering::SeqCst), 1);
    assert_eq!(connection.responses.load(Ordering::SeqCst), 0);
    let snapshot = app.get_session(target(&session)).await.unwrap();
    assert_eq!(snapshot.interactions.len(), 1);
    assert!(!delivered(&store, &session.session.session_id, &request).await);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn deletion_intent_blocks_connection_for_pending_feedback() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_deletions(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Deleting"))
        .await
        .unwrap();
    app.stop_session(target(&session)).await.unwrap();
    let request = submit_review(&store, &session).await;
    store
        .begin_managed_session_deletion(&session.session.session_id, "2026-10-03T00:00:00Z")
        .await
        .unwrap();
    app.start_delivery_worker().await.unwrap();
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 1);
    assert!(!delivered(&store, &session.session.session_id, &request).await);
    assert!(
        app.get_feedback_status(target(&session))
            .await
            .unwrap()
            .deleting
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn concurrent_explicit_start_and_pending_feedback_share_one_initialization() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Concurrent"))
        .await
        .unwrap();
    app.stop_session(target(&session)).await.unwrap();
    driver.hang.store(true, Ordering::SeqCst);
    app.start_delivery_worker().await.unwrap();
    let request = submit_review(&store, &session).await;
    eventually(|| async { driver.starting_count.load(Ordering::SeqCst) == 1 }).await;
    let explicit_start = {
        let app = app.clone();
        let input = target(&session);
        tokio::spawn(async move { app.start_session(input).await })
    };
    driver.hang.store(false, Ordering::SeqCst);
    driver.start_release.notify_one();
    assert_eq!(
        explicit_start.await.unwrap().unwrap().runtime.connection,
        SessionConnectionState::Connected
    );
    eventually(|| delivered(&store, &session.session.session_id, &request)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(driver.starts.lock().unwrap().len(), 2);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn stop_keeps_an_observed_unclaimed_job_offline_until_new_feedback_arrives() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Busy"))
        .await
        .unwrap();
    let barrier = app
        .create_session(input(&dir, &config, "Barrier"))
        .await
        .unwrap();
    let connection = driver.connections.lock().unwrap()[0].clone();
    connection.hold_prompt.store(true, Ordering::SeqCst);
    app.send_prompt(SendManagedPromptInput {
        session_id: session.session.session_id.clone(),
        text: "Existing work".into(),
    })
    .await
    .unwrap();
    app.start_delivery_worker().await.unwrap();
    let old_pending = submit_review(&store, &session).await;
    let barrier_request = submit_review(&store, &barrier).await;
    // This completed job proves the worker already observed the busy job.
    eventually(|| delivered(&store, &barrier.session.session_id, &barrier_request)).await;
    assert!(!delivered(&store, &session.session.session_id, &old_pending).await);
    app.stop_session(target(&session)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(
        app.get_feedback_status(target(&session))
            .await
            .unwrap()
            .connection,
        SessionConnectionState::Stopped
    );
    let fresh_pending = submit_review(&store, &session).await;
    eventually(|| async {
        delivered(&store, &session.session.session_id, &old_pending).await
            && delivered(&store, &session.session.session_id, &fresh_pending).await
    })
    .await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 3);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn disabled_configuration_is_visible_only_on_its_pending_session_and_can_be_retried() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let stopped = app
        .create_session(input(&dir, &config, "Disabled"))
        .await
        .unwrap();
    let connected = app
        .create_session(input(&dir, &config, "Already connected"))
        .await
        .unwrap();
    app.stop_session(target(&stopped)).await.unwrap();
    let mut disabled = store.get_agent_config(&config).await.unwrap();
    disabled.enabled = false;
    store.save_agent_config(disabled.clone()).await.unwrap();
    let pending = submit_review(&store, &stopped).await;
    app.start_delivery_worker().await.unwrap();
    eventually(|| async {
        app.get_feedback_status(target(&stopped))
            .await
            .unwrap()
            .connection
            == SessionConnectionState::Failed
    })
    .await;
    let snapshot = app.get_session(target(&stopped)).await.unwrap();
    assert!(
        snapshot
            .runtime
            .last_error
            .as_deref()
            .unwrap()
            .contains("disabled")
    );
    assert_eq!(snapshot.runtime.instance_id, None);
    assert_eq!(snapshot.deliveries[0].state, FeedbackDeliveryState::Pending);
    assert_eq!(snapshot.deliveries[0].attempt_id, None);
    assert_eq!(snapshot.deliveries[0].last_error, None);
    let other = submit_review(&store, &connected).await;
    eventually(|| delivered(&store, &connected.session.session_id, &other)).await;
    let unaffected = app.get_session(target(&connected)).await.unwrap();
    assert_eq!(
        unaffected.runtime.connection,
        SessionConnectionState::Connected
    );
    assert_eq!(unaffected.runtime.last_error, None);
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    disabled.enabled = true;
    store.save_agent_config(disabled).await.unwrap();
    app.start_session(target(&stopped)).await.unwrap();
    eventually(|| delivered(&store, &stopped.session.session_id, &pending)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 3);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn slow_initialization_does_not_block_another_connected_session_and_shutdown_joins_it() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let slow = app
        .create_session(input(&dir, &config, "Slow"))
        .await
        .unwrap();
    let connected = app
        .create_session(input(&dir, &config, "Connected"))
        .await
        .unwrap();
    app.stop_session(target(&slow)).await.unwrap();
    driver.hang.store(true, Ordering::SeqCst);
    app.start_delivery_worker().await.unwrap();
    let waiting = submit_review(&store, &slow).await;
    eventually(|| async { driver.starting_count.load(Ordering::SeqCst) == 1 }).await;
    let other = submit_review(&store, &connected).await;
    eventually(|| delivered(&store, &connected.session.session_id, &other)).await;
    assert_eq!(driver.starting_count.load(Ordering::SeqCst), 1);
    assert!(!delivered(&store, &slow.session.session_id, &waiting).await);
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 3);
    tokio::time::timeout(Duration::from_secs(5), app.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(driver.starting_count.load(Ordering::SeqCst), 0);
    store.close().await;
}

#[tokio::test]
async fn stop_before_first_worker_observation_suppresses_old_feedback_but_new_submission_resumes() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Stop first"))
        .await
        .unwrap();
    let old = submit_review(&store, &session).await;
    app.stop_session(target(&session)).await.unwrap();
    app.start_delivery_worker().await.unwrap();
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 1);
    let stopped = app.get_session(target(&session)).await.unwrap();
    assert_eq!(stopped.runtime.connection, SessionConnectionState::Stopped);
    assert_eq!(stopped.runtime.last_error, None);
    assert!(!delivered(&store, &session.session.session_id, &old).await);
    let new = submit_review(&store, &session).await;
    eventually(|| async {
        delivered(&store, &session.session.session_id, &old).await
            && delivered(&store, &session.session.session_id, &new).await
    })
    .await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn explicit_start_allows_delivery_of_feedback_suppressed_by_stop() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Explicit retry"))
        .await
        .unwrap();
    let pending = submit_review(&store, &session).await;
    app.stop_session(target(&session)).await.unwrap();
    app.start_delivery_worker().await.unwrap();
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 1);
    app.start_session(target(&session)).await.unwrap();
    eventually(|| delivered(&store, &session.session.session_id, &pending)).await;
    assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 2);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn new_submission_immediately_after_interrupted_initialization_keeps_its_admission() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Immediate resume"))
        .await
        .unwrap();
    app.start_delivery_worker().await.unwrap();
    for round in 0..3 {
        app.stop_session(target(&session)).await.unwrap();
        driver.hang.store(true, Ordering::SeqCst);
        let old = submit_review(&store, &session).await;
        eventually(|| async { driver.starting_count.load(Ordering::SeqCst) == 1 }).await;
        app.stop_session(target(&session)).await.unwrap();
        driver.hang.store(false, Ordering::SeqCst);
        // No idle poll or startup-task join barrier between Stop and submission.
        let new = submit_review(&store, &session).await;
        eventually(|| async {
            delivered(&store, &session.session.session_id, &old).await
                && delivered(&store, &session.session.session_id, &new).await
        })
        .await;
        assert_eq!(driver.start_attempts.load(Ordering::SeqCst), 3 + round * 2);
    }
    app.shutdown().await.unwrap();
    store.close().await;
}
