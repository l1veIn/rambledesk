use std::time::Duration;

use rambledesk_core::kernel::{AccessMode, LaunchConfiguration};
use serde_json::{Value, json};

use crate::{
    AccessModeTransport, AcpClientError, AcpErrorCode, ConfigOptionSelector, LaunchProfile,
    rpc::RpcPeer,
};

pub(super) fn supported_access_modes(
    profile: &LaunchProfile,
    config_options: &[Value],
) -> Vec<AccessMode> {
    let policy = &profile.configuration.access_mode;
    if policy.transport == AccessModeTransport::ImplicitWorkspaceWrite {
        return vec![AccessMode::WorkspaceWrite];
    }
    if policy.transport == AccessModeTransport::ProcessArguments {
        return [
            (!policy.read_only.is_empty()).then_some(AccessMode::ReadOnly),
            (!policy.workspace_write.is_empty()).then_some(AccessMode::WorkspaceWrite),
            (!policy.yolo.is_empty()).then_some(AccessMode::Yolo),
        ]
        .into_iter()
        .flatten()
        .collect();
    }
    let Some(selector) = policy.selector.as_ref() else {
        return Vec::new();
    };
    let Some(option) = find_option(config_options, selector) else {
        return Vec::new();
    };
    let mut supported = Vec::new();
    if any_select_value(option, &policy.read_only) {
        supported.push(AccessMode::ReadOnly);
    }
    if any_select_value(option, &policy.workspace_write) {
        supported.push(AccessMode::WorkspaceWrite);
    }
    if any_select_value(option, &policy.yolo) {
        supported.push(AccessMode::Yolo);
    }
    supported
}

pub(super) async fn apply_launch_configuration(
    rpc: &RpcPeer,
    acp_session_id: &str,
    profile: &LaunchProfile,
    launch: &LaunchConfiguration,
    mut config_options: Vec<Value>,
    timeout: Duration,
) -> Result<Vec<Value>, AcpClientError> {
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

pub(super) fn apply_process_launch_configuration(
    profile: &mut LaunchProfile,
    access_mode: AccessMode,
) -> Result<(), AcpClientError> {
    let policy = &profile.configuration.access_mode;
    if policy.transport != AccessModeTransport::ProcessArguments {
        return Ok(());
    }
    let arguments = match access_mode {
        AccessMode::ReadOnly => &policy.read_only,
        AccessMode::WorkspaceWrite => &policy.workspace_write,
        AccessMode::Yolo => &policy.yolo,
    };
    if arguments.is_empty() {
        return Err(AcpClientError::new(
            AcpErrorCode::UnsupportedAccessMode,
            format!("Launch Profile does not map {access_mode:?} for this Agent"),
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

fn any_select_value(option: &Value, candidates: &[String]) -> bool {
    candidates
        .iter()
        .any(|candidate| option_supports_value(option, candidate))
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
    fn codex_preflight_never_advertises_a_fake_read_only_mode() {
        let profile = LaunchProfile::codex_npx();
        let modes = supported_access_modes(
            &profile,
            &[json!({
                "id":"mode", "category":"mode", "type":"select", "options":[
                    {"value":"read-only"}, {"value":"agent"}, {"value":"agent-full-access"}
                ]
            })],
        );
        assert_eq!(modes, vec![AccessMode::WorkspaceWrite, AccessMode::Yolo]);
    }

    #[test]
    fn grok_process_modes_are_advertised_and_prepended_before_the_acp_subcommand() {
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
        assert_eq!(
            supported_access_modes(&profile, &[]),
            vec![
                AccessMode::ReadOnly,
                AccessMode::WorkspaceWrite,
                AccessMode::Yolo
            ]
        );

        apply_process_launch_configuration(&mut profile, AccessMode::Yolo)
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

    #[test]
    fn code_buddy_exposes_only_its_approval_gated_default() {
        let spec = crate::builtin_agent("code_buddy").expect("CodeBuddy catalog entry");
        let profile = LaunchProfile::for_builtin(
            spec,
            PathBuf::from("codebuddy"),
            Vec::new(),
            BTreeMap::new(),
        );
        assert_eq!(
            supported_access_modes(&profile, &[]),
            vec![AccessMode::WorkspaceWrite]
        );
    }
}
