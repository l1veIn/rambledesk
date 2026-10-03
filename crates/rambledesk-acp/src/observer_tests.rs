use super::*;
use rambledesk_core::{AgentDriverError, SessionInteractionKind};
use serde_json::json;

#[derive(Default)]
struct Sink(Mutex<Vec<AgentSessionEvent>>);

#[async_trait]
impl AgentSessionObserver for Sink {
    async fn observe(&self, event: AgentSessionEvent) -> Result<(), AgentDriverError> {
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}

fn observer() -> (ManagedObserver, Arc<Sink>) {
    let sink = Arc::new(Sink::default());
    (
        ManagedObserver {
            sink: sink.clone(),
            remote: Mutex::new(Some("remote".into())),
            local_session_id: "local".into(),
        },
        sink,
    )
}

fn permission(remote: &str, tool: serde_json::Value) -> AcpEvent {
    AcpEvent::PermissionRequested {
        request_id: "permission".into(),
        request: Box::new(
            serde_json::from_value(json!({
                "sessionId": remote, "toolCall": tool,
                "options": [{"optionId":"allow-once", "name":"Allow once", "kind":"allow_once"},
                            {"optionId":"reject-once", "name":"Reject", "kind":"reject_once"}]
            }))
            .unwrap(),
        ),
    }
}

#[tokio::test]
async fn id_only_permission_preserves_identity_and_does_not_guess_details() {
    let (observer, sink) = observer();
    observer
        .observe(permission("remote", json!({"toolCallId":"exact-tool"})))
        .await
        .unwrap();
    let events = sink.0.lock().unwrap();
    assert_eq!(events.len(), 2);
    let AgentSessionEvent::ToolCall {
        tool_call_id,
        patch,
    } = &events[0]
    else {
        panic!()
    };
    assert_eq!(tool_call_id, "exact-tool");
    assert_eq!(patch, &Default::default());
    let AgentSessionEvent::InteractionRequested(request) = &events[1] else {
        panic!()
    };
    assert_eq!(request.session_id, "local");
    assert_eq!(request.details, None);
    assert!(request.title.is_empty());
    let SessionInteractionKind::Permission {
        tool_call_id,
        options,
    } = &request.kind
    else {
        panic!()
    };
    assert_eq!(tool_call_id.as_deref(), Some("exact-tool"));
    assert_eq!(
        options
            .iter()
            .map(|option| option.option_id.as_str())
            .collect::<Vec<_>>(),
        vec!["allow-once", "reject-once"]
    );
}

#[tokio::test]
async fn explicit_permission_fields_update_the_tool_before_requesting_consent() {
    let (observer, sink) = observer();
    observer.observe(permission("remote", json!({
        "toolCallId":"tool", "title":"Current operation", "rawInput":{"command":"feedback get --request-id probe"},
        "content":[], "locations":[]
    }))).await.unwrap();
    let events = sink.0.lock().unwrap();
    let AgentSessionEvent::ToolCall { patch, .. } = &events[0] else {
        panic!()
    };
    assert_eq!(patch.title.as_deref(), Some("Current operation"));
    assert_eq!(patch.content, Some(vec![]));
    assert_eq!(patch.locations, Some(vec![]));
    let AgentSessionEvent::InteractionRequested(request) = &events[1] else {
        panic!()
    };
    assert_eq!(request.title, "Current operation");
    assert!(
        request
            .details
            .as_deref()
            .unwrap()
            .contains("feedback get --request-id probe")
    );
}

#[tokio::test]
async fn foreign_or_invalid_permission_identity_is_rejected_without_a_card() {
    for (remote, id) in [("foreign", "tool"), ("remote", "")] {
        let (observer, sink) = observer();
        assert!(
            observer
                .observe(permission(remote, json!({"toolCallId":id})))
                .await
                .is_err()
        );
        assert!(sink.0.lock().unwrap().is_empty());
    }
}
