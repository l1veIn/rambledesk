use super::*;
use agent_client_protocol::JsonRpcMessage;
use serde_json::json;
fn form(schema: Value) -> Result<InputSpec, AcpError> {
    InputSpec::parse(
        &form::FORM,
        &json!({"scope":{"sessionId":"session"},"requestedSchema":schema}),
        None,
    )
}
fn accept(content: Value) -> SessionInputResponse {
    SessionInputResponse {
        action: SessionInputAction::Accept,
        content: Some(content),
    }
}

#[test]
fn request_method_determines_question_or_plan_independently_of_title() {
    let fixtures: [(&str, &InputProtocol, Value, SessionInputKind); 5] = [
        (
            "elicitation/create",
            &form::FORM,
            json!({"sessionId":"session","message":"Approve this plan","requestedSchema":{"type":"object","properties":{}}}),
            SessionInputKind::Question,
        ),
        (
            "_x.ai/ask_user_question",
            &grok::QUESTION,
            json!({"sessionId":"session","questions":[{"question":"Approve this plan?","options":[{"label":"Yes"}]}]}),
            SessionInputKind::Question,
        ),
        (
            "_x.ai/exit_plan_mode",
            &grok::PLAN,
            json!({"sessionId":"session","planContent":"What should we do?"}),
            SessionInputKind::Plan,
        ),
        (
            "cursor/ask_question",
            &cursor::QUESTION,
            json!({"title":"Review plan","questions":[{"id":"choice","prompt":"Approve this plan?","options":[{"id":"yes","label":"Yes"}]}]}),
            SessionInputKind::Question,
        ),
        (
            "cursor/create_plan",
            &cursor::PLAN,
            json!({"name":"Agent question","plan":"What should we do?"}),
            SessionInputKind::Plan,
        ),
    ];
    let methods = [
        form::Elicitation(Value::Null).method().to_owned(),
        grok::Question(Value::Null).method().to_owned(),
        grok::Plan(Value::Null).method().to_owned(),
        cursor::Question(Value::Null).method().to_owned(),
        cursor::Plan(Value::Null).method().to_owned(),
    ];
    for ((method, protocol, raw, kind), registered_method) in fixtures.into_iter().zip(methods) {
        assert_eq!(method, registered_method);
        let spec = InputSpec::parse(protocol, &raw, Some("session")).unwrap();
        assert_eq!(spec.kind, kind, "{method}");
    }
}

#[test]
fn form_validates_multiselect_values_booleans_and_required_fields() {
    let spec=form(json!({"type":"object","properties":{"targets":{"type":"array","items":{"anyOf":[{"const":"linux","title":"Linux"},{"const":"windows","title":"Windows"}]},"minItems":1,"maxItems":2,"uniqueItems":true},"confirmed":{"type":"boolean"}},"required":["targets","confirmed"]})).unwrap();
    assert!(
        spec.answer(&accept(
            json!({"targets":["linux","windows"],"confirmed":true})
        ))
        .is_ok()
    );
    for invalid in [
        json!({"targets":["invented"],"confirmed":true}),
        json!({"targets":["linux","linux"],"confirmed":true}),
        json!({"targets":[],"confirmed":true}),
        json!({"targets":["linux"],"confirmed":"true"}),
        json!({"targets":["linux"]}),
    ] {
        assert!(spec.answer(&accept(invalid)).is_err());
    }
}

#[test]
fn unsupported_constraints_and_ambiguous_grok_questions_do_not_weaken_validation() {
    assert!(
        form(json!({"type":"object","properties":{},"oneOf":[{"pattern":"^trusted$"}]}))
            .unwrap()
            .answer(&accept(json!({})))
            .is_err()
    );
    assert!(
        form(json!({"type":"object","properties":{"x":{"type":"string","pattern":"^trusted$"}}}))
            .unwrap()
            .answer(&accept(json!({"x":"untrusted"})))
            .is_err()
    );
    assert!(
        form(json!({"type":"object","properties":{"x":{"type":"object","properties":{}}}}))
            .unwrap()
            .answer(&accept(json!({"x":{}})))
            .is_err()
    );
    let question = json!({"question":"Same?","options":[{"label":"Yes"},{"label":"No"}]});
    assert!(
        InputSpec::parse(
            &grok::QUESTION,
            &json!({"sessionId":"session","questions":[question.clone(),question]}),
            None,
        )
        .is_err()
    );
}

#[test]
fn keep_planning_cannot_silently_discard_revision_notes() {
    let spec = InputSpec::parse(
        &grok::PLAN,
        &json!({"sessionId":"session","planContent":null}),
        None,
    )
    .unwrap();
    assert!(
        spec.answer(&accept(
            json!({"decision":"keep_planning","feedback":"Change the design"})
        ))
        .is_err()
    );
    assert_eq!(
        spec.answer(&accept(json!({"decision":"keep_planning"})))
            .unwrap(),
        json!({"outcome":"keep_planning","feedback":""})
    );
}
