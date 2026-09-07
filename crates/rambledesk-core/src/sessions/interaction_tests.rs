use super::*;

fn question(kind: SessionInputKind) -> SessionInteraction {
    let input = SessionInputRequest {
        schema: serde_json::json!({"type":"object","properties":{}}),
    };
    SessionInteraction {
        request_id: "request".into(),
        session_id: "session".into(),
        title: "Question".into(),
        details: None,
        kind: match kind {
            SessionInputKind::Question => SessionInteractionKind::Question { input },
            SessionInputKind::Plan => SessionInteractionKind::Plan { input },
        },
    }
}

#[test]
fn interaction_responses_preserve_semantics_even_when_cancelled() {
    let question = question(SessionInputKind::Question);
    let cancel = SessionInputResponse {
        action: SessionInputAction::Cancel,
        content: None,
    };
    assert!(!question.allows(&SessionInteractionResponse::Permission { option_id: None }));
    assert!(!question.allows(&SessionInteractionResponse::Plan {
        response: cancel.clone()
    }));
    assert!(question.allows(&SessionInteractionResponse::Question { response: cancel }));
    assert!(question.allows(&question.cancel_response()));
}

#[test]
fn interaction_contract_has_one_tagged_payload_and_rejects_mixed_responses() {
    let plan = question(SessionInputKind::Plan);
    let value = serde_json::to_value(plan).unwrap();
    assert_eq!(value["kind"], "plan");
    assert!(value.get("input").is_some());
    assert!(value.get("options").is_none());
    assert!(
        serde_json::from_value::<SessionInteractionResponse>(serde_json::json!({
            "kind":"permission", "option_id":null, "response":{"action":"accept","content":{}}
        }))
        .is_err()
    );
}
