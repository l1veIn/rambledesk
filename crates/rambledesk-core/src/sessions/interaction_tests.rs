use super::*;

fn question(kind: SessionInputKind) -> SessionInteraction {
    let input = SessionInputRequest {
        schema_json: r#"{"type":"object","properties":{}}"#.into(),
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
        content_json: None,
    };
    assert!(!question.allows(&SessionInteractionResponse::Permission { option_id: None }));
    assert!(!question.allows(&SessionInteractionResponse::Plan {
        response: cancel.clone()
    }));
    assert!(question.allows(&SessionInteractionResponse::Question { response: cancel }));
    assert!(question.allows(&question.cancel_response()));
}
