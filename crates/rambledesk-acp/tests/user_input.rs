mod support;
use rambledesk_core::*;
use serde_json::{Value, json};
use support::{create, id, setup, wait_for};

#[test]
fn interaction_responses_reject_unknown_fields() {
    for mut payload in [
        serde_json::json!({ "kind": "permission", "option_id": null }),
        serde_json::json!({
            "kind": "question",
            "response": { "action": "cancel", "content_json": null }
        }),
        serde_json::json!({
            "kind": "plan",
            "response": { "action": "cancel", "content_json": null }
        }),
    ] {
        assert!(serde_json::from_value::<SessionInteractionResponse>(payload.clone()).is_ok());
        payload["unexpected"] = serde_json::json!(true);
        let error = serde_json::from_value::<SessionInteractionResponse>(payload).unwrap_err();
        assert!(error.to_string().contains("unknown field `unexpected`"));
    }
}

#[tokio::test]
async fn native_inputs_are_pending_validated_and_answered_on_the_original_wire() {
    for (kind, content, expected) in [
        (
            "cursor",
            json!({"platform":"desktop"}),
            json!({"outcome":{"outcome":"answered","answers":[{"questionId":"platform","selectedOptionIds":["desktop"]}]}}),
        ),
        (
            "cursor_plan",
            json!({"decision":"accepted"}),
            json!({"outcome":{"outcome":"accepted"}}),
        ),
        (
            "form",
            json!({"target":"desktop","count":2}),
            json!({"action":"accept","content":{"target":"desktop","count":2}}),
        ),
        (
            "grok",
            json!({"Which target?":"My custom target"}),
            json!({"outcome":"accepted","answers":{"Which target?":"My custom target"},"partial_answers":{}}),
        ),
        (
            "plan",
            json!({"decision":"approved","feedback":"Include tests"}),
            json!({"outcome":"approved","feedback":"Include tests"}),
        ),
    ] {
        let (dir, store, app, config) = setup("user_input", kind).await;
        let session = create(&app, &dir, &config, "Input").await;
        app.send_prompt(SendManagedPromptInput {
            session_id: session.session.session_id.clone(),
            text: kind.into(),
        })
        .await
        .unwrap();
        let pending = wait_for(&app, &id(&session), |s| {
            !s.interactions.is_empty() || s.runtime.activity == SessionActivityState::Idle
        })
        .await;
        assert_eq!(
            pending.interactions.len(),
            1,
            "native request must wait for an answer: {kind}"
        );
        let request = &pending.interactions[0];
        let input_kind = if matches!(kind, "plan" | "cursor_plan") {
            "plan"
        } else {
            "question"
        };
        assert_eq!(serde_json::to_value(request).unwrap()["kind"], input_kind);
        assert_eq!(pending.runtime.activity, SessionActivityState::WaitingInput);
        assert!(
            serde_json::from_str::<Value>(
                serde_json::to_value(request).unwrap()["input"]["schema_json"]
                    .as_str()
                    .unwrap()
            )
            .unwrap()
            .is_object()
        );
        let answer = |content: Value| {
            serde_json::from_value::<RespondManagedInteractionInput>(json!({"session_id":session.session.session_id,"request_id":request.request_id,"response":{"kind":input_kind,"response":{"action":"accept","content_json":content.to_string()}}})).unwrap()
        };
        let mut wrong_kind = answer(content.clone());
        let valid_input = SessionInputResponse {
            action: SessionInputAction::Accept,
            content_json: Some(content.to_string()),
        };
        wrong_kind.response = if input_kind == "plan" {
            SessionInteractionResponse::Question {
                response: valid_input,
            }
        } else {
            SessionInteractionResponse::Plan {
                response: valid_input,
            }
        };
        assert!(app.respond_interaction(wrong_kind).await.is_err());
        assert_eq!(
            app.get_session(id(&session))
                .await
                .unwrap()
                .interactions
                .len(),
            1
        );
        assert!(
            app.respond_interaction(answer(json!({"invented":"value"})))
                .await
                .is_err()
        );
        if kind == "form" {
            for bad in [
                json!({"target":"invented","count":2}),
                json!({"target":"desktop","count":0}),
                json!({"target":"desktop","count":"2"}),
            ] {
                assert!(app.respond_interaction(answer(bad)).await.is_err());
            }
        }
        let response = answer(content);
        app.respond_interaction(response.clone()).await.unwrap();
        assert!(app.respond_interaction(response).await.is_err());
        let done = wait_for(&app, &id(&session), |s| {
            s.runtime.activity == SessionActivityState::Idle
        })
        .await;
        assert!(done.interactions.is_empty());
        assert!(
            done.activities
                .iter()
                .any(|a| a.text.contains(&expected.to_string())),
            "actual wire answer must match: {kind}"
        );
        app.shutdown().await.unwrap();
        store.close().await;
    }
}

#[tokio::test]
async fn unsupported_forms_stay_visible_and_can_be_declined_without_weakening_constraints() {
    let (dir, store, app, config) = setup("user_input", "unsupported").await;
    let session = create(&app, &dir, &config, "Unsupported form").await;
    app.send_prompt(SendManagedPromptInput {
        session_id: session.session.session_id.clone(),
        text: "unsupported".into(),
    })
    .await
    .unwrap();
    let pending = wait_for(&app, &id(&session), |s| {
        !s.interactions.is_empty() || s.runtime.activity == SessionActivityState::Idle
    })
    .await;
    assert_eq!(
        pending.interactions.len(),
        1,
        "unsupported form must remain actionable"
    );
    assert_eq!(
        serde_json::from_str::<Value>(
            serde_json::to_value(&pending.interactions[0]).unwrap()["input"]["schema_json"]
                .as_str()
                .unwrap()
        )
        .unwrap()["x-rambledesk-unsupported"],
        true
    );
    let mut response = RespondManagedInteractionInput {
        session_id: session.session.session_id.clone(),
        request_id: pending.interactions[0].request_id.clone(),
        response: SessionInteractionResponse::Question {
            response: SessionInputResponse {
                action: SessionInputAction::Accept,
                content_json: Some(json!({"secret":"trusted"}).to_string()),
            },
        },
    };
    assert!(app.respond_interaction(response.clone()).await.is_err());
    response.response = SessionInteractionResponse::Question {
        response: SessionInputResponse {
            action: SessionInputAction::Decline,
            content_json: None,
        },
    };
    app.respond_interaction(response).await.unwrap();
    let done = wait_for(&app, &id(&session), |s| {
        s.runtime.activity == SessionActivityState::Idle
    })
    .await;
    assert!(
        done.activities
            .iter()
            .any(|a| a.text.contains("\"action\":\"decline\""))
    );
    app.shutdown().await.unwrap();
    store.close().await;
}

#[tokio::test]
async fn late_requests_are_cancelled_on_the_wire_instead_of_parking_invisibly() {
    for (kind, expected) in [
        ("late_input", json!({"action":"cancel"})),
        (
            "late_permission",
            json!({"outcome":{"outcome":"cancelled"}}),
        ),
    ] {
        let (dir, store, app, config) = setup("user_input", kind).await;
        let log = dir.path().join("late-reply.jsonl");
        let mut saved = store.get_agent_config(&config).await.unwrap();
        saved.env.insert(
            "FIXTURE_INPUT_LOG".into(),
            log.to_string_lossy().into_owned(),
        );
        store.save_agent_config(saved).await.unwrap();
        let session = create(&app, &dir, &config, "Late request").await;
        app.send_prompt(SendManagedPromptInput {
            session_id: session.session.session_id.clone(),
            text: kind.into(),
        })
        .await
        .unwrap();
        let reply = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if let Ok(text) = std::fs::read_to_string(&log) {
                    break text;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("late request was left unanswered");
        assert_eq!(
            serde_json::from_str::<Value>(reply.trim()).unwrap(),
            expected
        );
        assert!(
            app.get_session(id(&session))
                .await
                .unwrap()
                .interactions
                .is_empty()
        );
        app.shutdown().await.unwrap();
        store.close().await;
    }
}

#[tokio::test]
async fn cancelling_native_inputs_returns_provider_cancellation_and_consumes_the_request() {
    for (kind, expected) in [
        ("cursor", json!({"outcome":{"outcome":"cancelled"}})),
        ("cursor_plan", json!({"outcome":{"outcome":"cancelled"}})),
        ("form", json!({"action":"cancel"})),
        ("grok", json!({"outcome":"skip_interview"})),
        ("plan", json!({"outcome":"keep_planning","feedback":""})),
    ] {
        let (dir, store, app, config) = setup("user_input", kind).await;
        let session = create(&app, &dir, &config, "Cancellation").await;
        let other = create(&app, &dir, &config, "Other session").await;
        app.send_prompt(SendManagedPromptInput {
            session_id: session.session.session_id.clone(),
            text: kind.into(),
        })
        .await
        .unwrap();
        let pending = wait_for(&app, &id(&session), |s| !s.interactions.is_empty()).await;
        let request = &pending.interactions[0];
        let answer = RespondManagedInteractionInput {
            session_id: session.session.session_id.clone(),
            request_id: request.request_id.clone(),
            response: request.cancel_response(),
        };
        assert!(
            app.respond_interaction(RespondManagedInteractionInput {
                session_id: other.session.session_id.clone(),
                ..answer.clone()
            })
            .await
            .is_err()
        );
        assert!(
            app.respond_interaction(RespondManagedInteractionInput {
                response: SessionInteractionResponse::Permission {
                    option_id: Some("approved".into())
                },
                ..answer.clone()
            })
            .await
            .is_err()
        );
        app.respond_interaction(answer.clone()).await.unwrap();
        assert!(app.respond_interaction(answer).await.is_err());
        let done = wait_for(&app, &id(&session), |s| {
            s.runtime.activity == SessionActivityState::Idle
        })
        .await;
        assert!(
            done.activities
                .iter()
                .any(|a| a.text.contains(&expected.to_string()))
        );
        app.shutdown().await.unwrap();
        store.close().await;
    }
}
