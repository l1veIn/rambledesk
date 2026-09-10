//! Standard form elicitation preserves unsupported schemas for explicit refusal.
use super::*;
use agent_client_protocol::JsonRpcRequest;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "elicitation/create", response = Value)]
pub(super) struct Elicitation(pub Value);

pub(super) static FORM: InputProtocol = InputProtocol {
    kind: SessionInputKind::Question,
    decode,
    accept: |content| Ok(json!({"action":"accept","content":content})),
    cancel: |action| json!({"action":if action == SessionInputAction::Decline {"decline"} else {"cancel"}}),
};

fn decode(raw: &Value, _: Option<&str>) -> Result<InputFields, AcpError> {
    let remote = raw
        .pointer("/scope/sessionId")
        .or_else(|| raw.get("sessionId"))
        .and_then(Value::as_str)
        .ok_or(AcpError::InvalidPermission)?
        .to_owned();
    if raw
        .get("mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode != "form")
    {
        return Err(AcpError::InvalidPermission);
    }
    Ok(InputFields {
        remote,
        title: raw
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Agent question")
            .into(),
        schema: raw
            .get("requestedSchema")
            .cloned()
            .ok_or(AcpError::InvalidPermission)?,
    })
}

pub(super) fn register<H: HandleDispatchFrom<Agent>>(
    builder: Builder<Client, H>,
    queue: Queue,
    observer: Observer,
) -> Builder<Client, impl HandleDispatchFrom<Agent>> {
    builder.on_receive_request(
        async move |request: Elicitation, responder, _| {
            receive(&FORM, request.0, responder, &queue, &observer).await
        },
        agent_client_protocol::on_receive_request!(),
    )
}
