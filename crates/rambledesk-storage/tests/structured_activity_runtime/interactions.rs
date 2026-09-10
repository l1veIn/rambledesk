use super::*;

fn request(id: &str, kind: SessionInteractionKind) -> SessionInteraction {
    SessionInteraction {
        request_id: "reused-id".into(),
        session_id: id.into(),
        title: "Review".into(),
        details: None,
        kind,
    }
}

#[tokio::test]
async fn old_response_cannot_retire_a_new_instance_interaction() {
    for next in ["completed", "next_turn", "reconnected"] {
        let (_dir, store, app, driver, ids) = setup().await;
        let id = &ids[0];
        prompt(&app, id).await;
        let old = driver.connections.lock().unwrap()[id].clone();
        let item = request(id, SessionInteractionKind::Permission { options: vec![] });
        old.observer
            .observe(AgentSessionEvent::InteractionRequested(item.clone()))
            .await
            .unwrap();
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        *old.response_gate.lock().unwrap() = Some((entered.clone(), release.clone()));
        let responding = app.clone();
        let response = RespondManagedInteractionInput {
            session_id: id.clone(),
            request_id: item.request_id.clone(),
            response: item.cancel_response(),
        };
        let task = tokio::spawn(async move { responding.respond_interaction(response).await });
        tokio::time::timeout(std::time::Duration::from_secs(3), entered.notified())
            .await
            .unwrap();
        if next == "reconnected" {
            app.stop_session(ManagedSessionInput {
                session_id: id.clone(),
            })
            .await
            .unwrap();
            app.start_session(ManagedSessionInput {
                session_id: id.clone(),
            })
            .await
            .unwrap();
        } else {
            old.finish.notify_one();
            idle(&app, id).await;
        }
        if next != "completed" {
            prompt(&app, id).await;
            let current = driver.connections.lock().unwrap()[id].clone();
            current
                .observer
                .observe(AgentSessionEvent::InteractionRequested(item))
                .await
                .unwrap();
        }
        release.notify_one();
        let response = task.await.unwrap();
        if next == "completed" {
            assert!(
                response.is_ok(),
                "a completed answered turn is a successful reply"
            );
        } else {
            assert!(matches!(response, Err(SessionError::Interrupted)));
        }
        let snapshot = app
            .get_session(ManagedSessionInput {
                session_id: id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(
            snapshot.interactions.len(),
            usize::from(next != "completed")
        );
        assert_eq!(
            snapshot.runtime.activity,
            if next == "completed" {
                SessionActivityState::Idle
            } else {
                SessionActivityState::WaitingInput
            }
        );
        app.shutdown().await.unwrap();
        store.close().await;
    }
}

#[tokio::test]
async fn finishing_a_turn_cancels_each_visible_pending_request_with_its_kind() {
    let (_dir, store, app, driver, ids) = setup().await;
    let id = &ids[0];
    prompt(&app, id).await;
    let connection = driver.connections.lock().unwrap()[id].clone();
    let item = request(
        id,
        SessionInteractionKind::Plan {
            input: SessionInputRequest {
                schema_json: serde_json::json!({"type":"object","properties":{}}).to_string(),
            },
        },
    );
    let expected = item.cancel_response();
    connection
        .observer
        .observe(AgentSessionEvent::InteractionRequested(item))
        .await
        .unwrap();
    connection.finish.notify_one();
    idle(&app, id).await;
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if !connection.responses.lock().unwrap().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("finishing cleared the card but left the request pending");
    assert_eq!(*connection.responses.lock().unwrap(), vec![expected]);
    assert!(
        app.get_session(ManagedSessionInput {
            session_id: id.clone()
        })
        .await
        .unwrap()
        .interactions
        .is_empty()
    );
    app.shutdown().await.unwrap();
    store.close().await;
}
