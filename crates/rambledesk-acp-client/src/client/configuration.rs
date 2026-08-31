use std::time::Duration;

use rambledesk_core::kernel::{AccessMode, LaunchConfiguration};
use serde_json::{Value, json};

use crate::{
    AccessModeTransport, AcpClientError, AcpErrorCode, ConfigOptionSelector, LaunchConfigSelection,
    LaunchProfile,
    launch_schema::{
        PROFILE_ACCESS_OPTION_ID, decode_agent_launch_config, project_launch_schema,
        validate_selections,
    },
    rpc::RpcPeer,
};

pub(super) async fn apply_launch_configuration(
    rpc: &RpcPeer,
    acp_session_id: &str,
    profile: &LaunchProfile,
    launch: &LaunchConfiguration,
    mut config_options: Vec<Value>,
    timeout: Duration,
) -> Result<Vec<Value>, AcpClientError> {
    if let Some(agent_config) = decode_agent_launch_config(&launch.agent_config_json) {
        let (schema, _) = project_launch_schema(profile, &config_options);
        validate_selections(&agent_config.schema_digest, &schema, &agent_config.values)
            .map_err(AcpClientError::invalid)?;
        return apply_generic_launch_configuration(
            rpc,
            acp_session_id,
            agent_config.values,
            config_options,
            timeout,
        )
        .await;
    }

    let mut selections = Vec::new();
    if let (Some(model), Some(selector)) = (
        launch.model.as_deref(),
        profile.configuration.model.as_ref(),
    ) {
        selections.push(ConfigSelection::exact("model", selector, model));
    }
    if let (Some(effort), Some(selector)) = (
        launch.reasoning_effort.as_deref(),
        profile.configuration.reasoning_effort.as_ref(),
    ) {
        selections.push(ConfigSelection::exact("reasoning effort", selector, effort));
    }
    match profile.configuration.access_mode.transport {
        AccessModeTransport::ConfigOption => selections.push(access_selection(profile, launch)?),
        AccessModeTransport::ImplicitWorkspaceWrite
            if launch.access_mode != AccessMode::WorkspaceWrite =>
        {
            return Err(AcpClientError::new(
                AcpErrorCode::UnsupportedAccessMode,
                "this Agent exposes only its approval-gated default Access Mode",
                false,
            ));
        }
        AccessModeTransport::ImplicitWorkspaceWrite | AccessModeTransport::ProcessArguments => {}
    }

    for selection in selections {
        let option = find_option(&config_options, selection.selector).ok_or_else(|| {
            unsupported(format!(
                "Agent did not return a config option for {}",
                selection.label
            ))
        })?;
        let selected = selection.resolve_value(option)?;
        if option.get("currentValue") == Some(&Value::String(selected.to_owned())) {
            continue;
        }
        let mutation = option.get("_rambledeskMutation").and_then(Value::as_str);
        match mutation {
            Some("set_model") => {
                rpc.request(
                    "session/set_model",
                    json!({"sessionId": acp_session_id, "modelId": selected}),
                    Some(timeout),
                )
                .await?;
                set_current_value(&mut config_options, selection.selector, selected);
            }
            Some("set_mode") => {
                rpc.request(
                    "session/set_mode",
                    json!({"sessionId": acp_session_id, "modeId": selected}),
                    Some(timeout),
                )
                .await?;
                set_current_value(&mut config_options, selection.selector, selected);
            }
            _ => {
                let config_id = required_string(option, "id")?;
                let response = rpc
                    .request(
                        "session/set_config_option",
                        json!({
                            "sessionId": acp_session_id,
                            "configId": config_id,
                            "value": selected
                        }),
                        Some(timeout),
                    )
                    .await?;
                config_options = project_config_options(
                    response
                        .get("configOptions")
                        .and_then(Value::as_array)
                        .cloned()
                        .ok_or_else(|| {
                            AcpClientError::protocol(
                                "session/set_config_option response omitted configOptions",
                            )
                        })?,
                );
            }
        }
    }
    Ok(config_options)
}

async fn apply_generic_launch_configuration(
    rpc: &RpcPeer,
    acp_session_id: &str,
    selections: Vec<LaunchConfigSelection>,
    mut config_options: Vec<Value>,
    timeout: Duration,
) -> Result<Vec<Value>, AcpClientError> {
    for selection in selections {
        if selection.id == PROFILE_ACCESS_OPTION_ID {
            continue;
        }
        let option = config_options
            .iter()
            .find(|option| option.get("id").and_then(Value::as_str) == Some(&selection.id))
            .ok_or_else(|| {
                unsupported(format!(
                    "Agent did not return config option {}",
                    selection.id
                ))
            })?;
        validate_generic_value(option, &selection)?;
        if option.get("currentValue") == Some(&selection.value) {
            continue;
        }
        let mutation = option.get("_rambledeskMutation").and_then(Value::as_str);
        match mutation {
            Some("set_model") | Some("set_mode") => {
                let selected = selection.value.as_str().ok_or_else(|| {
                    unsupported(format!("{} requires a string value", selection.id))
                })?;
                let (method, params) = if mutation == Some("set_model") {
                    (
                        "session/set_model",
                        json!({"sessionId": acp_session_id, "modelId": selected}),
                    )
                } else {
                    (
                        "session/set_mode",
                        json!({"sessionId": acp_session_id, "modeId": selected}),
                    )
                };
                rpc.request(method, params, Some(timeout)).await?;
                set_current_value_by_id(&mut config_options, &selection.id, &selection.value);
            }
            _ => {
                let response = rpc
                    .request(
                        "session/set_config_option",
                        json!({
                            "sessionId": acp_session_id,
                            "configId": selection.id,
                            "value": selection.value
                        }),
                        Some(timeout),
                    )
                    .await?;
                config_options = project_config_options(
                    response
                        .get("configOptions")
                        .and_then(Value::as_array)
                        .cloned()
                        .ok_or_else(|| {
                            AcpClientError::protocol(
                                "session/set_config_option response omitted configOptions",
                            )
                        })?,
                );
            }
        }
    }
    Ok(config_options)
}

fn validate_generic_value(
    option: &Value,
    selection: &LaunchConfigSelection,
) -> Result<(), AcpClientError> {
    match option.get("type").and_then(Value::as_str) {
        Some("select") if option_supports_json_value(option, &selection.value) => Ok(()),
        Some("select") => Err(unsupported(format!(
            "Agent config option {} no longer offers the selected value",
            selection.id
        ))),
        Some("boolean") if selection.value.is_boolean() => Ok(()),
        Some("boolean") => Err(unsupported(format!(
            "Agent config option {} requires a boolean value",
            selection.id
        ))),
        _ => Err(unsupported(format!(
            "Agent config option {} has an unsupported type",
            selection.id
        ))),
    }
}

pub(super) fn apply_process_launch_configuration(
    profile: &mut LaunchProfile,
    launch: &LaunchConfiguration,
) -> Result<(), AcpClientError> {
    let policy = &profile.configuration.access_mode;
    if let Some(agent_config) = decode_agent_launch_config(&launch.agent_config_json) {
        let selected =
            match agent_config
                .values
                .iter()
                .find(|selection| selection.id == PROFILE_ACCESS_OPTION_ID)
            {
                Some(selection) => Some(selection.value.as_str().ok_or_else(|| {
                    AcpClientError::invalid("profile Access Mode must be a string")
                })?),
                None => None,
            };
        return match policy.transport {
            AccessModeTransport::ProcessArguments => {
                let selected = selected.ok_or_else(|| {
                    AcpClientError::invalid("profile Access Mode selection is missing")
                })?;
                apply_process_access_value(profile, selected)
            }
            AccessModeTransport::ImplicitWorkspaceWrite => match selected {
                Some("workspace_write") => Ok(()),
                _ => Err(AcpClientError::new(
                    AcpErrorCode::UnsupportedAccessMode,
                    "this Agent exposes only its approval-gated default Access Mode",
                    false,
                )),
            },
            AccessModeTransport::ConfigOption => Ok(()),
        };
    }
    if policy.transport != AccessModeTransport::ProcessArguments {
        return Ok(());
    }
    let legacy_value = match launch.access_mode {
        AccessMode::ReadOnly => "read_only",
        AccessMode::WorkspaceWrite => "workspace_write",
        AccessMode::Yolo => "yolo",
    };
    apply_process_access_value(profile, legacy_value)
}

fn apply_process_access_value(
    profile: &mut LaunchProfile,
    selected: &str,
) -> Result<(), AcpClientError> {
    let policy = &profile.configuration.access_mode;
    let arguments = match selected {
        "read_only" => &policy.read_only,
        "workspace_write" => &policy.workspace_write,
        "yolo" => &policy.yolo,
        _ => {
            return Err(AcpClientError::new(
                AcpErrorCode::UnsupportedAccessMode,
                format!("Launch Profile does not map {selected} for this Agent"),
                false,
            ));
        }
    };
    let access_label = match selected {
        "read_only" => AccessMode::ReadOnly,
        "workspace_write" => AccessMode::WorkspaceWrite,
        "yolo" => AccessMode::Yolo,
        _ => unreachable!("validated above"),
    };
    if arguments.is_empty() {
        return Err(AcpClientError::new(
            AcpErrorCode::UnsupportedAccessMode,
            format!("Launch Profile does not map {access_label:?} for this Agent"),
            false,
        ));
    }
    profile.args.splice(0..0, arguments.iter().cloned());
    Ok(())
}

#[derive(Debug)]
struct ConfigSelection<'a> {
    label: &'static str,
    selector: &'a ConfigOptionSelector,
    values: Vec<&'a str>,
}

impl<'a> ConfigSelection<'a> {
    fn exact(label: &'static str, selector: &'a ConfigOptionSelector, value: &'a str) -> Self {
        Self {
            label,
            selector,
            values: vec![value],
        }
    }

    fn resolve_value<'b>(&self, option: &'b Value) -> Result<&'a str, AcpClientError> {
        if option.get("type").and_then(Value::as_str) != Some("select") {
            return Err(unsupported("Agent config option is not a select option"));
        }
        self.values
            .iter()
            .copied()
            .find(|candidate| option_supports_value(option, candidate))
            .ok_or_else(|| {
                unsupported(format!(
                    "Agent config option does not offer any mapped value for {}",
                    self.label
                ))
            })
    }
}

fn access_selection<'a>(
    profile: &'a LaunchProfile,
    launch: &LaunchConfiguration,
) -> Result<ConfigSelection<'a>, AcpClientError> {
    let policy = &profile.configuration.access_mode;
    let selector = policy.selector.as_ref().ok_or_else(|| {
        AcpClientError::new(
            AcpErrorCode::UnsupportedAccessMode,
            "this Launch Profile does not define an Access Mode mapping",
            false,
        )
    })?;
    let values = match launch.access_mode {
        AccessMode::ReadOnly => &policy.read_only,
        AccessMode::WorkspaceWrite => &policy.workspace_write,
        AccessMode::Yolo => &policy.yolo,
    };
    if values.is_empty() {
        return Err(AcpClientError::new(
            AcpErrorCode::UnsupportedAccessMode,
            format!(
                "Launch Profile does not map {:?} for this Agent",
                launch.access_mode
            ),
            false,
        ));
    }
    Ok(ConfigSelection {
        label: "Access Mode",
        selector,
        values: values.iter().map(String::as_str).collect(),
    })
}

fn find_option<'a>(options: &'a [Value], selector: &ConfigOptionSelector) -> Option<&'a Value> {
    options.iter().find(|option| {
        option
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| selector.ids.iter().any(|candidate| candidate == id))
            || option
                .get("category")
                .and_then(Value::as_str)
                .is_some_and(|category| {
                    selector
                        .categories
                        .iter()
                        .any(|candidate| candidate == category)
                })
    })
}

fn option_supports_value(option: &Value, selected: &str) -> bool {
    option
        .get("options")
        .and_then(Value::as_array)
        .is_some_and(|options| select_contains(options, selected))
}

fn option_supports_json_value(option: &Value, selected: &Value) -> bool {
    option
        .get("options")
        .and_then(Value::as_array)
        .is_some_and(|options| select_contains_json(options, selected))
}

fn set_current_value(options: &mut [Value], selector: &ConfigOptionSelector, selected: &str) {
    if let Some(option) = options
        .iter_mut()
        .find(|option| option_matches(option, selector))
        .and_then(Value::as_object_mut)
    {
        option.insert(
            "currentValue".to_owned(),
            Value::String(selected.to_owned()),
        );
    }
}

fn set_current_value_by_id(options: &mut [Value], id: &str, selected: &Value) {
    if let Some(option) = options
        .iter_mut()
        .find(|option| option.get("id").and_then(Value::as_str) == Some(id))
        .and_then(Value::as_object_mut)
    {
        option.insert("currentValue".to_owned(), selected.clone());
    }
}

fn option_matches(option: &Value, selector: &ConfigOptionSelector) -> bool {
    option
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| selector.ids.iter().any(|candidate| candidate == id))
        || option
            .get("category")
            .and_then(Value::as_str)
            .is_some_and(|category| {
                selector
                    .categories
                    .iter()
                    .any(|candidate| candidate == category)
            })
}

fn select_contains(options: &[Value], selected: &str) -> bool {
    options.iter().any(|candidate| {
        candidate.get("value").and_then(Value::as_str) == Some(selected)
            || candidate
                .get("options")
                .and_then(Value::as_array)
                .is_some_and(|children| select_contains(children, selected))
    })
}

fn select_contains_json(options: &[Value], selected: &Value) -> bool {
    options.iter().any(|candidate| {
        candidate.get("value") == Some(selected)
            || candidate
                .get("options")
                .and_then(Value::as_array)
                .is_some_and(|children| select_contains_json(children, selected))
    })
}

pub(super) fn project_config_options(mut options: Vec<Value>) -> Vec<Value> {
    for option in &mut options {
        let Some(object) = option.as_object_mut() else {
            continue;
        };
        let Some(grouped) = object.get("options").and_then(Value::as_array).cloned() else {
            continue;
        };
        if grouped.is_empty()
            || !grouped
                .iter()
                .all(|group| group.get("options").is_some_and(Value::is_array))
        {
            continue;
        }
        let flattened = grouped
            .iter()
            .flat_map(|group| {
                group
                    .get("options")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .cloned()
            })
            .collect();
        object.insert("optionGroups".to_string(), Value::Array(grouped));
        object.insert("options".to_string(), Value::Array(flattened));
    }
    options
}

fn required_string(value: &Value, field: &str) -> Result<String, AcpClientError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AcpClientError::protocol(format!("config option omitted {field}")))
}

fn unsupported(message: impl Into<String>) -> AcpClientError {
    AcpClientError::new(AcpErrorCode::UnsupportedCapability, message, false)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::*;

    #[test]
    fn codex_read_only_is_rejected_instead_of_silently_widened() {
        let launch = LaunchConfiguration {
            agent_profile_id: "codex".to_string(),
            launch_profile_id: "codex-acp-npx".to_string(),
            workspace_reference: "/tmp".to_string(),
            model: None,
            reasoning_effort: None,
            access_mode: AccessMode::ReadOnly,
            agent_config_json: "{}".to_string(),
        };
        let error = access_selection(&LaunchProfile::codex_npx(), &launch)
            .expect_err("read-only must not be widened");
        assert_eq!(error.code, AcpErrorCode::UnsupportedAccessMode);
    }

    #[test]
    fn grouped_selects_are_validated_and_flattened_for_preflight_projection() {
        let mut options = vec![json!({
            "id":"model", "category":"model", "type":"select", "currentValue":"openai/gpt-5",
            "options":[{
                "group":"openai", "name":"OpenAI", "options":[
                    {"value":"openai/gpt-5","name":"GPT-5"},
                    {"value":"openai/gpt-5-mini","name":"GPT-5 Mini"}
                ]
            }]
        })];
        assert!(option_supports_value(&options[0], "openai/gpt-5-mini"));
        options = project_config_options(options);
        assert_eq!(options[0]["options"].as_array().unwrap().len(), 2);
        assert_eq!(options[0]["optionGroups"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn grok_process_access_is_a_generic_profile_option_and_legacy_launches_still_map_it() {
        let spec = crate::builtin_agent("grok").expect("grok catalog entry");
        let mut profile = LaunchProfile::for_builtin(
            spec,
            PathBuf::from("grok"),
            vec![
                "--no-auto-update".to_owned(),
                "agent".to_owned(),
                "stdio".to_owned(),
            ],
            BTreeMap::new(),
        );
        let (schema, _) = crate::launch_schema::project_launch_schema(&profile, &[]);
        let access = schema.last().expect("synthetic process Access option");
        assert_eq!(access.id, crate::launch_schema::PROFILE_ACCESS_OPTION_ID);
        assert_eq!(access.source, crate::LaunchConfigSource::Profile);
        let crate::LaunchConfigKind::Select { options, .. } = &access.kind else {
            panic!("process Access option must be selectable")
        };
        assert_eq!(
            options
                .iter()
                .map(|option| option.value.as_str())
                .collect::<Vec<_>>(),
            ["read_only", "workspace_write", "yolo"]
        );

        apply_process_launch_configuration(
            &mut profile,
            &LaunchConfiguration {
                agent_profile_id: "grok".to_owned(),
                launch_profile_id: "grok-acp-managed".to_owned(),
                workspace_reference: "/tmp".to_owned(),
                model: None,
                reasoning_effort: None,
                access_mode: AccessMode::Yolo,
                agent_config_json: "{}".to_owned(),
            },
        )
        .expect("mapped process mode");
        assert_eq!(
            profile.args,
            [
                "--permission-mode",
                "bypassPermissions",
                "--no-auto-update",
                "agent",
                "stdio"
            ]
        );
    }
}
