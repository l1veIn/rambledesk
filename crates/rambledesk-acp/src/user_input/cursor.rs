//! Cursor's documented blocking extensions. Option identifiers remain opaque.
use super::*;
use agent_client_protocol::JsonRpcRequest;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "cursor/ask_question", response = Value)]
pub(super) struct Question(pub Value);
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "cursor/create_plan", response = Value)]
pub(super) struct Plan(pub Value);

pub(super) static QUESTION: InputProtocol = InputProtocol {
    kind: SessionInputKind::Question,
    decode: |raw, active| fields(raw, active, question_schema(raw)?),
    accept: answer,
    cancel: |action| json!({"outcome":{"outcome":if action == SessionInputAction::Decline {"skipped"} else {"cancelled"}}}),
};
pub(super) static PLAN: InputProtocol = InputProtocol {
    kind: SessionInputKind::Plan,
    decode: |raw, active| fields(raw, active, plan_schema(raw)?),
    accept: |_| Ok(json!({"outcome":{"outcome":"accepted"}})),
    cancel: |action| json!({"outcome":{"outcome":if action == SessionInputAction::Decline {"rejected"} else {"cancelled"}}}),
};

fn fields(
    raw: &Value,
    active: Option<&str>,
    (title, schema): (String, Value),
) -> Result<InputFields, AcpError> {
    // Cursor omits sessionId. Its connection owns one managed remote session;
    // never let an optional payload field override that attribution.
    raw.as_object().ok_or(AcpError::InvalidPermission)?;
    let remote = active.ok_or(AcpError::InvalidPermission)?.to_owned();
    check_schema(&schema, 0)?;
    Ok(InputFields {
        remote,
        title,
        schema,
    })
}

pub(super) fn register<H: HandleDispatchFrom<Agent>>(
    builder: Builder<Client, H>,
    queue: Queue,
    observer: Observer,
) -> Builder<Client, impl HandleDispatchFrom<Agent>> {
    let plan_queue = queue.clone();
    let plan_observer = observer.clone();
    builder
        .on_receive_request(
            async move |request: Question, responder, _| {
                receive(&QUESTION, request.0, responder, &queue, &observer).await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: Plan, responder, _| {
                receive(&PLAN, request.0, responder, &plan_queue, &plan_observer).await
            },
            agent_client_protocol::on_receive_request!(),
        )
}

pub(super) fn question_schema(raw: &Value) -> Result<(String, Value), AcpError> {
    let questions = raw
        .get("questions")
        .and_then(Value::as_array)
        .filter(|q| !q.is_empty() && q.len() <= 32)
        .ok_or(AcpError::InvalidPermission)?;
    let mut properties = Map::new();
    let mut required = Vec::new();
    for question in questions {
        let id = question
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or(AcpError::InvalidPermission)?;
        if properties.contains_key(id) {
            return Err(AcpError::InvalidPermission);
        }
        let options = question
            .get("options")
            .and_then(Value::as_array)
            .filter(|o| !o.is_empty() && o.len() <= 128)
            .ok_or(AcpError::InvalidPermission)?;
        let mut choices = Vec::new();
        for option in options {
            let value = option
                .get("id")
                .and_then(Value::as_str)
                .ok_or(AcpError::InvalidPermission)?;
            let label = option
                .get("label")
                .and_then(Value::as_str)
                .ok_or(AcpError::InvalidPermission)?;
            if value.is_empty() || choices.iter().any(|c: &Value| c["const"] == value) {
                return Err(AcpError::InvalidPermission);
            }
            choices.push(json!({"const":value,"title":label}));
        }
        let title = question.get("prompt").and_then(Value::as_str).unwrap_or(id);
        let property = if question
            .get("allowMultiple")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            json!({"type":"array","title":title,"items":{"type":"string","anyOf":choices},"minItems":1,"uniqueItems":true})
        } else {
            json!({"type":"string","title":title,"oneOf":choices})
        };
        properties.insert(id.into(), property);
        required.push(id);
    }
    Ok((
        raw.get("title")
            .and_then(Value::as_str)
            .unwrap_or("Agent question")
            .into(),
        json!({"type":"object","properties":properties,"required":required}),
    ))
}

pub(super) fn plan_schema(raw: &Value) -> Result<(String, Value), AcpError> {
    Ok((
        raw.get("name")
            .and_then(Value::as_str)
            .unwrap_or("Review plan")
            .into(),
        json!({"type":"object","description":raw.get("plan").and_then(Value::as_str).ok_or(AcpError::InvalidPermission)?,"properties":{"decision":{"type":"string","title":"Decision","oneOf":[{"const":"accepted","title":"Approve and implement"}]}},"required":["decision"]}),
    ))
}

pub(super) fn answer(content: &Value) -> Result<Value, AcpError> {
    let answers = content
        .as_object()
        .ok_or(AcpError::InvalidPermission)?
        .iter()
        .map(|(id, value)| {
            let selected = if value.is_array() {
                value.clone()
            } else {
                json!([value])
            };
            json!({"questionId":id,"selectedOptionIds":selected})
        })
        .collect::<Vec<_>>();
    Ok(json!({"outcome":{"outcome":"answered","answers":answers}}))
}
