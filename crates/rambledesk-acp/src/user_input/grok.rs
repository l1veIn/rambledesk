//! Grok extensions use distinct interview and plan outcomes.
use super::*;
use agent_client_protocol::JsonRpcRequest;
use serde::{Deserialize, Serialize};
use serde_json::{Map, json};

#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "_x.ai/ask_user_question", response = Value)]
pub(super) struct Question(pub Value);
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcRequest)]
#[request(method = "_x.ai/exit_plan_mode", response = Value)]
pub(super) struct Plan(pub Value);

pub(super) static QUESTION: InputProtocol = InputProtocol {
    kind: SessionInputKind::Question,
    decode: |raw, _| fields(raw, "Agent question", question_schema(raw)?),
    accept: |content| Ok(json!({"outcome":"accepted","answers":content,"partial_answers":{}})),
    cancel: |_| json!({"outcome":"skip_interview"}),
};
pub(super) static PLAN: InputProtocol = InputProtocol {
    kind: SessionInputKind::Plan,
    decode: |raw, _| fields(raw, "Review plan", plan_schema(raw)),
    accept: answer_plan,
    cancel: |_| json!({"outcome":"keep_planning","feedback":""}),
};

fn fields(raw: &Value, title: &str, schema: Value) -> Result<InputFields, AcpError> {
    let remote = raw
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or(AcpError::InvalidPermission)?
        .to_owned();
    check_schema(&schema, 0)?;
    Ok(InputFields {
        remote,
        title: title.into(),
        schema,
    })
}

fn question_schema(raw: &Value) -> Result<Value, AcpError> {
    let questions = raw
        .get("questions")
        .and_then(Value::as_array)
        .filter(|questions| !questions.is_empty() && questions.len() <= 32)
        .ok_or(AcpError::InvalidPermission)?;
    let mut properties = Map::new();
    let mut required = Vec::new();
    for question in questions {
        let text = question
            .get("question")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .ok_or(AcpError::InvalidPermission)?;
        if properties.contains_key(text) {
            return Err(AcpError::InvalidPermission);
        }
        let options = question
            .get("options")
            .and_then(Value::as_array)
            .filter(|options| !options.is_empty() && options.len() <= 128)
            .ok_or(AcpError::InvalidPermission)?;
        let mut choices = Vec::new();
        for option in options {
            let label = option
                .get("label")
                .and_then(Value::as_str)
                .filter(|label| !label.trim().is_empty())
                .ok_or(AcpError::InvalidPermission)?;
            if choices
                .iter()
                .any(|choice: &Value| choice["const"] == label)
            {
                return Err(AcpError::InvalidPermission);
            }
            choices.push(json!({"const":label,"title":label,"description":option.get("description").and_then(Value::as_str).unwrap_or("")}));
        }
        let choices = json!({"type":"string","oneOf":choices,"x-rambledesk-allow-other":true});
        let property = if question
            .get("multiSelect")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            json!({"type":"array","title":text,"items":choices,"minItems":1,"uniqueItems":true,"x-rambledesk-allow-other":true})
        } else {
            choices
        };
        properties.insert(text.into(), property);
        required.push(text);
    }
    Ok(json!({"type":"object","properties":properties,"required":required}))
}

fn plan_schema(raw: &Value) -> Value {
    json!({
        "type":"object",
        "description":raw.get("planContent").and_then(Value::as_str).unwrap_or(""),
        "properties":{
            "decision":{"type":"string","title":"Decision","oneOf":[{"const":"approved","title":"Approve and implement"},{"const":"keep_planning","title":"Keep planning"},{"const":"abandoned","title":"Abandon plan"}]},
            "feedback":{"type":"string","title":"Comments for approval","description":"Feedback accompanies approval only. To request changes, choose Keep planning and send your revision notes in the next message.","maxLength":16384}
        },"required":["decision"]
    })
}

fn answer_plan(content: &Value) -> Result<Value, AcpError> {
    let feedback = content
        .get("feedback")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    // Grok discards feedback on keep_planning. Do not silently lose revision notes.
    if content["decision"] != "approved" && !feedback.is_empty() {
        return Err(AcpError::InvalidPermission);
    }
    Ok(json!({"outcome":content["decision"],"feedback":feedback}))
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
