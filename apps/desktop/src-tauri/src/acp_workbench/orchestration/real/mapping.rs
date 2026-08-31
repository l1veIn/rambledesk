use rambledesk_acp_client::{AskFieldKind, AskQuestion, PermissionRequest, PreflightReport};
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::{ProjectedPermission, ProjectedQuestion, QuestionBinding};
use crate::acp_workbench::model::{
    AttentionItem, AttentionStatus, LaunchPreflight, PermissionOption, PermissionTone,
    QuestionChoice,
};

pub(super) fn project_permission(
    request: &PermissionRequest,
    created_at: &str,
) -> ProjectedPermission {
    let tool_title = string_at(&request.request_meta, &[&["permission", "title"]])
        .or_else(|| string_at(&request.tool_call, &[&["title"], &["name"]]))
        .unwrap_or_else(|| "Permission required".to_owned());
    let description = string_at(&request.request_meta, &[&["permission", "description"]])
        .or_else(|| string_at(&request.tool_call, &[&["description"], &["kind"]]))
        .unwrap_or_else(|| "The Agent is waiting for your permission.".to_owned());
    let command = string_at(
        &request.tool_call,
        &[
            &["rawInput", "command"],
            &["raw_input", "command"],
            &["input", "command"],
            &["command"],
        ],
    );
    let path = string_at(
        &request.tool_call,
        &[
            &["rawInput", "path"],
            &["raw_input", "path"],
            &["input", "path"],
            &["path"],
        ],
    );
    let options = request
        .options
        .iter()
        .map(|option| PermissionOption {
            id: option.option_id.clone(),
            label: option.name.clone(),
            tone: permission_tone(&option.kind),
        })
        .collect();
    ProjectedPermission {
        item: AttentionItem::Permission {
            id: request.live_request_id.clone(),
            session_id: request.session_id.to_string(),
            title: tool_title.clone(),
            created_at: created_at.to_owned(),
            status: AttentionStatus::Waiting,
            description,
            tool_call: request.tool_call.clone(),
            tool_title,
            command,
            path,
            options,
        },
        queue_position: request.queue_position,
    }
}

fn permission_tone(kind: &str) -> PermissionTone {
    let normalized = kind.to_ascii_lowercase();
    if normalized.contains("reject") || normalized.contains("deny") {
        PermissionTone::Deny
    } else if normalized.contains("allow") || normalized.contains("accept") {
        PermissionTone::Allow
    } else {
        PermissionTone::Neutral
    }
}

pub(super) fn project_question(question: &AskQuestion, created_at: &str) -> ProjectedQuestion {
    if question.fields.len() == 1 {
        let field = &question.fields[0];
        if matches!(
            field.kind,
            AskFieldKind::SingleSelect | AskFieldKind::MultiSelect
        ) && !field.options.is_empty()
        {
            let multiple = field.kind == AskFieldKind::MultiSelect;
            let choices = field
                .options
                .iter()
                .enumerate()
                .map(|(index, option)| QuestionChoice {
                    id: format!("choice-{index}"),
                    label: option.label.clone(),
                    description: None,
                })
                .collect();
            let bindings = field
                .options
                .iter()
                .enumerate()
                .map(|(index, option)| (format!("choice-{index}"), option.value.clone()))
                .collect();
            return ProjectedQuestion {
                item: AttentionItem::Question {
                    id: question.live_request_id.clone(),
                    session_id: question.session_id.to_string(),
                    title: field.title.clone(),
                    created_at: created_at.to_owned(),
                    status: AttentionStatus::Waiting,
                    prompt: question.message.clone(),
                    choices,
                    multiple,
                    allow_skip: true,
                    unsupported_reason: None,
                },
                binding: QuestionBinding::Select {
                    session_id: question.session_id.clone(),
                    live_request_id: question.live_request_id.clone(),
                    field_id: field.field_id.clone(),
                    multiple,
                    choices: bindings,
                },
                queue_position: question.queue_position,
            };
        }
    }

    let received = question
        .fields
        .iter()
        .map(|field| format!("{} ({:?})", field.title, field.kind))
        .collect::<Vec<_>>()
        .join(", ");
    let reason = format!(
        "RambleDesk currently supports exactly one single-select or multi-select field; received {}.",
        if received.is_empty() {
            "no renderable fields".to_owned()
        } else {
            received
        }
    );
    tracing::warn!(
        live_request_id = %question.live_request_id,
        %reason,
        "ACP Ask Question shape is not supported by the Desktop UI"
    );
    ProjectedQuestion {
        item: AttentionItem::Question {
            id: question.live_request_id.clone(),
            session_id: question.session_id.to_string(),
            title: "Unsupported Ask Question".to_owned(),
            created_at: created_at.to_owned(),
            status: AttentionStatus::Waiting,
            prompt: format!(
                "{}\n\nThis question shape is not supported yet. You can skip it to decline. {reason}",
                question.message
            ),
            choices: Vec::new(),
            multiple: false,
            allow_skip: true,
            unsupported_reason: Some(reason.clone()),
        },
        binding: QuestionBinding::Unsupported {
            session_id: question.session_id.clone(),
            live_request_id: question.live_request_id.clone(),
            reason,
        },
        queue_position: question.queue_position,
    }
}

pub(super) fn project_preflight(agent_id: &str, report: &PreflightReport) -> LaunchPreflight {
    let mut models = Vec::new();
    let mut reasoning_efforts = Vec::new();
    let mut access_modes = report.supported_access_modes.clone();
    let mut warnings = report.warnings.clone();
    let mut unexposed_options = Vec::new();
    for option in &report.config_options {
        let id = option
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let category = option.get("category").and_then(Value::as_str);
        match id {
            "thought_level" | "reasoning_effort" | "reasoning" => {
                reasoning_efforts = select_values_current_first(option);
            }
            "model" => models = select_values_current_first(option),
            _ => match category {
                Some("model") => models = select_values_current_first(option),
                Some("thought_level") | Some("reasoning_effort") => {
                    reasoning_efforts = select_values_current_first(option);
                }
                Some("mode") => {}
                // Access Modes are projected from the Launch Profile's explicit
                // mapping in `PreflightReport`. Extra Agent options stay private
                // to the ACP client instead of making the simple Launch form look
                // partially broken.
                _ => unexposed_options.push(id.to_owned()),
            },
        }
    }
    if !unexposed_options.is_empty() {
        warnings.push(format!(
            "The agent also reported options RambleDesk does not expose: {}.",
            unexposed_options.join(", ")
        ));
    }
    deduplicate(&mut models);
    deduplicate(&mut reasoning_efforts);
    deduplicate(&mut access_modes);
    LaunchPreflight {
        agent_id: agent_id.to_owned(),
        models,
        reasoning_efforts,
        access_modes,
        warning: (!warnings.is_empty()).then(|| warnings.join(" ")),
    }
}

fn select_values_current_first(option: &Value) -> Vec<String> {
    if option.get("type").and_then(Value::as_str) != Some("select") {
        return Vec::new();
    }
    let current = option
        .get("currentValue")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let mut values = current.into_iter().collect::<Vec<_>>();
    values.extend(
        option
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value.get("value").and_then(Value::as_str))
            .map(ToOwned::to_owned),
    );
    deduplicate(&mut values);
    values
}

fn deduplicate<T: Eq>(values: &mut Vec<T>) {
    let mut index = 0;
    while index < values.len() {
        if values[..index].contains(&values[index]) {
            values.remove(index);
        } else {
            index += 1;
        }
    }
}

fn string_at(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let mut current = value;
        for segment in *path {
            current = current.get(*segment)?;
        }
        match current {
            Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
            Value::Array(parts) => {
                let text = parts
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" ");
                (!text.is_empty()).then_some(text)
            }
            _ => None,
        }
    })
}

pub(super) fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp().to_string())
}
