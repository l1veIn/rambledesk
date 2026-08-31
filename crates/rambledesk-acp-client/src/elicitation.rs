use std::collections::HashSet;

use serde_json::{Value, json};

use crate::{
    AcpClientError, AskField, AskFieldKind, AskOption, AskQuestion, PermissionOption,
    QuestionAction,
};

const MAX_FIELDS: usize = 16;
const MAX_TEXT_CHARS: usize = 4_096;
pub(crate) const DECLINE_OPTION_ID: &str = "__decline";

#[derive(Debug, Clone)]
pub(crate) enum ElicitationPlan {
    Decline,
    Question {
        question: AskQuestion,
        schema: Value,
    },
    Approval {
        message: String,
        tool_call_id: Option<String>,
        options: Vec<PermissionOption>,
        persist_in_content: bool,
    },
}

pub(crate) fn classify(
    params: &Value,
    session_id: rambledesk_core::kernel::SessionId,
    live_request_id: String,
    queue_position: usize,
) -> Result<ElicitationPlan, AcpClientError> {
    if params.get("mode").and_then(Value::as_str) != Some("form") {
        return Err(AcpClientError::protocol(
            "only advertised ACP form elicitation is accepted",
        ));
    }
    let message = bounded_text(Some(
        params.get("message").and_then(Value::as_str).unwrap_or(""),
    ));
    let schema = params
        .get("requestedSchema")
        .cloned()
        .ok_or_else(|| AcpClientError::protocol("form elicitation omitted requestedSchema"))?;
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(AcpClientError::protocol(
            "form elicitation schema must be a flat object",
        ));
    }
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| AcpClientError::protocol("form schema omitted properties"))?;
    if properties.len() > MAX_FIELDS {
        return Err(AcpClientError::protocol(format!(
            "form schema exceeds {MAX_FIELDS} fields"
        )));
    }
    let tool_call_id = params
        .get("toolCallId")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    if is_approval(params) {
        return Ok(approval_plan(
            message,
            tool_call_id,
            properties.get("persist"),
        ));
    }

    let required: HashSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut fields = Vec::new();
    for (field_id, property) in properties {
        if is_custom_answer_property(property) || is_other_companion(field_id) {
            continue;
        }
        if let Some(field) = parse_field(field_id, property, required.contains(field_id.as_str())) {
            fields.push(field);
        }
    }
    if fields.is_empty() && properties.is_empty() {
        return Ok(approval_plan(message, tool_call_id, None));
    }
    if fields.len() != 1
        || !matches!(
            fields[0].kind,
            AskFieldKind::SingleSelect | AskFieldKind::MultiSelect
        )
        || fields[0].options.is_empty()
    {
        return Ok(ElicitationPlan::Decline);
    }
    Ok(ElicitationPlan::Question {
        question: AskQuestion {
            live_request_id,
            session_id,
            tool_call_id,
            message,
            fields,
            queue_position,
        },
        schema,
    })
}

fn is_approval(params: &Value) -> bool {
    params
        .get("_meta")
        .and_then(|value| value.get("codex_approval_kind"))
        .and_then(Value::as_str)
        == Some("mcp_tool_call")
}

fn approval_plan(
    message: String,
    tool_call_id: Option<String>,
    persist_property: Option<&Value>,
) -> ElicitationPlan {
    let mut options = persist_property
        .map(choice_options)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|option| {
            let value = option.value.as_str()?.trim();
            if value.is_empty() || value == DECLINE_OPTION_ID {
                return None;
            }
            Some(PermissionOption {
                option_id: value.to_string(),
                name: option.label,
                kind: if value == "once" {
                    "allow_once".to_string()
                } else {
                    "allow_always".to_string()
                },
            })
        })
        .take(4)
        .collect::<Vec<_>>();
    let persist_in_content = !options.is_empty();
    if options.is_empty() {
        options.push(PermissionOption {
            option_id: "accept".to_string(),
            name: "Allow".to_string(),
            kind: "allow_once".to_string(),
        });
    }
    options.push(PermissionOption {
        option_id: DECLINE_OPTION_ID.to_string(),
        name: "Decline".to_string(),
        kind: "reject_once".to_string(),
    });
    ElicitationPlan::Approval {
        message,
        tool_call_id,
        options,
        persist_in_content,
    }
}

fn parse_field(field_id: &str, property: &Value, required: bool) -> Option<AskField> {
    let property_type = property.get("type").and_then(Value::as_str)?;
    let options = choice_options(property);
    let kind = match property_type {
        "string" if !options.is_empty() => AskFieldKind::SingleSelect,
        "string" => AskFieldKind::Text,
        "boolean" => AskFieldKind::Boolean,
        "number" => AskFieldKind::Number,
        "integer" => AskFieldKind::Integer,
        "array" => AskFieldKind::MultiSelect,
        _ => return None,
    };
    let options = if matches!(kind, AskFieldKind::Boolean) {
        vec![
            AskOption {
                label: "Yes".to_string(),
                value: Value::Bool(true),
            },
            AskOption {
                label: "No".to_string(),
                value: Value::Bool(false),
            },
        ]
    } else if matches!(kind, AskFieldKind::MultiSelect) {
        property
            .get("items")
            .map(choice_options)
            .unwrap_or_default()
    } else {
        options
    };
    Some(AskField {
        field_id: bounded_text(Some(field_id)),
        title: bounded_text(
            property
                .get("title")
                .and_then(Value::as_str)
                .or(Some(field_id)),
        ),
        description: property
            .get("description")
            .and_then(Value::as_str)
            .map(|value| bounded_text(Some(value))),
        kind,
        required,
        secret: property
            .get("_meta")
            .and_then(|value| value.get("codex"))
            .and_then(|value| value.get("isSecret"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        options,
    })
}

fn choice_options(property: &Value) -> Vec<AskOption> {
    if let Some(one_of) = property.get("oneOf").and_then(Value::as_array) {
        return one_of
            .iter()
            .filter_map(|item| {
                let value = item.get("const")?.clone();
                let label = item
                    .get("title")
                    .and_then(Value::as_str)
                    .map(|text| bounded_text(Some(text)))
                    .unwrap_or_else(|| bounded_text(value.as_str()));
                Some(AskOption { label, value })
            })
            .take(16)
            .collect();
    }
    property
        .get("enum")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(16)
        .cloned()
        .map(|value| AskOption {
            label: bounded_text(value.as_str()),
            value,
        })
        .collect()
}

fn is_custom_answer_property(property: &Value) -> bool {
    property
        .get("_meta")
        .and_then(|value| value.get("_askUserQuestionCustomAnswer"))
        .and_then(|value| value.get("isCustomAnswer"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn is_other_companion(field_id: &str) -> bool {
    field_id.rfind("__other").is_some_and(|position| {
        position > 0
            && field_id[position + "__other".len()..]
                .chars()
                .all(|value| value.is_ascii_digit())
    })
}

pub(crate) fn question_response(
    schema: &Value,
    action: QuestionAction,
    content: Option<Value>,
) -> Result<Value, AcpClientError> {
    match action {
        QuestionAction::Decline => Ok(json!({"action": "decline"})),
        QuestionAction::Cancel => Ok(json!({"action": "cancel"})),
        QuestionAction::Accept => {
            let content = content.ok_or_else(|| {
                AcpClientError::new(
                    crate::AcpErrorCode::InvalidLiveAnswer,
                    "accepted question requires content",
                    false,
                )
            })?;
            validate_content(schema, &content)?;
            Ok(json!({"action": "accept", "content": content}))
        }
    }
}

pub(crate) fn approval_response(option_id: &str, persist_in_content: bool) -> Value {
    if option_id == DECLINE_OPTION_ID {
        return json!({"action": "decline"});
    }
    if persist_in_content {
        json!({"action": "accept", "content": {"persist": option_id}})
    } else {
        json!({"action": "accept"})
    }
}

fn validate_content(schema: &Value, content: &Value) -> Result<(), AcpClientError> {
    let object = content.as_object().ok_or_else(|| {
        AcpClientError::new(
            crate::AcpErrorCode::InvalidLiveAnswer,
            "question answer content must be an object",
            false,
        )
    })?;
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<HashSet<_>>();
    for id in required {
        if !object.contains_key(id) {
            return Err(invalid_answer(format!("required field {id:?} is missing")));
        }
    }
    for (id, value) in object {
        let Some(property) = properties.get(id) else {
            return Err(invalid_answer(format!("unknown answer field {id:?}")));
        };
        validate_value(id, property, value)?;
    }
    Ok(())
}

fn validate_value(id: &str, property: &Value, value: &Value) -> Result<(), AcpClientError> {
    let valid_type = match property.get("type").and_then(Value::as_str) {
        Some("string") => value.is_string(),
        Some("boolean") => value.is_boolean(),
        Some("number") => value.is_number(),
        Some("integer") => value.as_i64().is_some() || value.as_u64().is_some(),
        Some("array") => value.is_array(),
        _ => false,
    };
    if !valid_type {
        return Err(invalid_answer(format!(
            "answer field {id:?} does not match its schema type"
        )));
    }
    let allowed = choice_options(property)
        .into_iter()
        .map(|option| option.value)
        .collect::<Vec<_>>();
    if !allowed.is_empty() && !allowed.contains(value) {
        return Err(invalid_answer(format!(
            "answer field {id:?} is outside its allowed values"
        )));
    }
    Ok(())
}

fn invalid_answer(message: String) -> AcpClientError {
    AcpClientError::new(crate::AcpErrorCode::InvalidLiveAnswer, message, false)
}

fn bounded_text(value: Option<&str>) -> String {
    value
        .unwrap_or("")
        .trim()
        .chars()
        .take(MAX_TEXT_CHARS)
        .collect()
}

#[cfg(test)]
mod tests {
    use rambledesk_core::kernel::SessionId;
    use serde_json::json;

    use super::*;

    #[test]
    fn question_form_maps_flat_schema_and_skips_other_companion() {
        let plan = classify(
            &json!({
                "sessionId": "agent-session",
                "mode": "form",
                "message": "Choose",
                "requestedSchema": {
                    "type": "object",
                    "properties": {
                        "strategy": {"type": "string", "enum": ["safe", "fast"]},
                        "strategy__other": {"type": "string"}
                    },
                    "required": ["strategy"]
                }
            }),
            SessionId::new("session-1"),
            "live-1".to_string(),
            0,
        )
        .unwrap();
        let ElicitationPlan::Question { question, .. } = plan else {
            panic!("expected question")
        };
        assert_eq!(question.fields.len(), 1);
        assert_eq!(question.fields[0].kind, AskFieldKind::SingleSelect);
    }

    #[test]
    fn codex_mcp_approval_uses_permission_shape() {
        let plan = classify(
            &json!({
                "sessionId": "agent-session",
                "mode": "form",
                "message": "Allow MCP tool?",
                "_meta": {"codex_approval_kind": "mcp_tool_call"},
                "requestedSchema": {
                    "type": "object",
                    "properties": {
                        "persist": {"type": "string", "oneOf": [
                            {"const": "once", "title": "Allow once"},
                            {"const": "always", "title": "Always allow"}
                        ]}
                    }
                }
            }),
            SessionId::new("session-2"),
            "live-2".to_string(),
            0,
        )
        .unwrap();
        let ElicitationPlan::Approval {
            options,
            persist_in_content,
            ..
        } = plan
        else {
            panic!("expected approval")
        };
        assert!(persist_in_content);
        assert_eq!(options.last().unwrap().option_id, DECLINE_OPTION_ID);
    }

    #[test]
    fn accepted_answer_is_validated_against_schema() {
        let schema = json!({
            "type": "object",
            "properties": {"choice": {"type": "string", "enum": ["a", "b"]}},
            "required": ["choice"]
        });
        assert!(
            question_response(
                &schema,
                QuestionAction::Accept,
                Some(json!({"choice": "a"}))
            )
            .is_ok()
        );
        assert!(
            question_response(
                &schema,
                QuestionAction::Accept,
                Some(json!({"choice": "c"}))
            )
            .is_err()
        );
    }

    #[test]
    fn unsupported_multi_field_or_free_text_forms_are_declined() {
        for properties in [
            json!({"details": {"type": "string"}}),
            json!({
                "first": {"type": "string", "enum": ["a", "b"]},
                "second": {"type": "string", "enum": ["c", "d"]}
            }),
        ] {
            let plan = classify(
                &json!({
                    "sessionId": "agent-session",
                    "mode": "form",
                    "message": "Unsupported",
                    "requestedSchema": {"type": "object", "properties": properties}
                }),
                SessionId::new("session-3"),
                "live-3".to_string(),
                0,
            )
            .unwrap();
            assert!(matches!(plan, ElicitationPlan::Decline));
        }
    }
}
