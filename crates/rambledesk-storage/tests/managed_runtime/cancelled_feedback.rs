use super::*;
use std::time::Duration;

async fn request_review(
    feedback: &FeedbackApplication,
    session: &ManagedSessionSnapshot,
) -> FeedbackRequestView {
    feedback
        .request_managed_feedback(
            &ManagedFeedbackScope::from_session(&session.session).unwrap(),
            RequestFeedbackInput {
                request_id: None,
                host_id: None,
                host_session_id: String::new(),
                title: Some("Review the work".into()),
                what_happened: "Ready for human review".into(),
                actions: vec![ActionInput {
                    id: "review".into(),
                    instruction: "Review the work".into(),
                }],
                context_refs: vec![],
                attachments: vec![],
                source_hint: None,
                allow_finish: true,
                final_summary: Some("The work is ready".into()),
            },
        )
        .await
        .unwrap()
}

async fn cancel_review(feedback: &FeedbackApplication, request: &FeedbackRequestView) {
    let cancelled = WorkbenchTerminalOperations::without_observer(feedback.clone())
        .cancel_feedback(CancelFeedbackInput {
            request_id: request.request_id.clone(),
            reason: "No feedback to send".into(),
        })
        .await
        .unwrap();
    assert_eq!(cancelled.status, FeedbackStatus::Cancelled);
}

#[tokio::test]
async fn cancellation_never_sends_continuation_but_approval_still_does() {
    let (dir, store, driver, app, config) = setup().await;
    let app = app.with_deliveries(store.clone());
    let session = app
        .create_session(input(&dir, &config, "Review"))
        .await
        .unwrap();
    let feedback = store.as_ref().clone().into_application();
    app.start_delivery_worker().await.unwrap();
    let cancelled = request_review(&feedback, &session).await;
    cancel_review(&feedback, &cancelled).await;
    cancel_review(&feedback, &cancelled).await;

    // A later real approval is a barrier proving the worker processed this queue.
    let approved = request_review(&feedback, &session).await;
    feedback
        .approve_feedback(ApproveFeedbackInput {
            request_id: approved.request_id.clone(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let deliveries = store
                .list_session_deliveries(&session.session.session_id)
                .await
                .unwrap();
            if deliveries.iter().any(|item| {
                item.request_id == approved.request_id
                    && item.state == FeedbackDeliveryState::Delivered
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(driver.starts.lock().unwrap().len(), 1);
    assert_eq!(
        driver.connections.lock().unwrap()[0]
            .prompts
            .load(Ordering::SeqCst),
        1
    );
    let snapshot = app.get_session(target(&session)).await.unwrap();
    let messages: Vec<_> = snapshot
        .activities
        .iter()
        .filter(|item| item.kind == SessionActivityKind::UserMessage)
        .collect();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].text.contains(&approved.request_id));
    assert!(!messages[0].text.contains(&cancelled.request_id));
    assert_eq!(snapshot.deliveries.len(), 1);
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn cancelled_feedback_stays_offline_after_restart_until_explicit_agent_use() {
    let (dir, store, _, app, config) = setup().await;
    let app = app
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    let original = app
        .create_session(input(&dir, &config, "Cancelled review"))
        .await
        .unwrap();
    let feedback = store.as_ref().clone().into_application();
    let request = request_review(&feedback, &original).await;
    cancel_review(&feedback, &request).await;
    app.shutdown().await.unwrap();

    let driver = Arc::new(FakeDriver::default());
    let restarted = SessionApplication::new(store.clone(), store.clone(), driver.clone())
        .with_deliveries(store.clone())
        .with_recovery(store.clone());
    restarted.start_delivery_worker().await.unwrap();
    let read = restarted.get_session(target(&original)).await.unwrap();
    let status = restarted
        .get_feedback_status(target(&original))
        .await
        .unwrap();
    assert_ne!(read.runtime.connection, SessionConnectionState::Connected);
    assert_ne!(status.connection, SessionConnectionState::Connected);
    assert!(read.deliveries.is_empty());
    assert!(driver.starts.lock().unwrap().is_empty());

    // Opening the Agent page may reconnect; only the user's new message is sent.
    let reopened = restarted.start_session(target(&original)).await.unwrap();
    assert_eq!(reopened.session.management, original.session.management);
    assert_eq!(driver.starts.lock().unwrap().len(), 1);
    let SessionManagement::Managed {
        remote_session_id, ..
    } = &original.session.management
    else {
        panic!("managed session")
    };
    assert_eq!(&driver.starts.lock().unwrap()[0].1, remote_session_id);
    restarted
        .send_prompt(SendManagedPromptInput {
            session_id: original.session.session_id.clone(),
            text: "Continue with my new instructions".into(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = restarted.get_session(target(&original)).await.unwrap();
            if snapshot.runtime.activity == SessionActivityState::Idle {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        driver.connections.lock().unwrap()[0]
            .prompts
            .load(Ordering::SeqCst),
        1
    );
    assert!(
        store
            .list_session_deliveries(&original.session.session_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        feedback
            .get_feedback(GetFeedbackInput {
                request_id: request.request_id
            })
            .await
            .unwrap()
            .status,
        FeedbackStatus::Cancelled
    );
    restarted.shutdown().await.unwrap();
    store.close().await;
}
