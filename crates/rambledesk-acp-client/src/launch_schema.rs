use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};

use crate::{
    AccessModeTransport, AgentLaunchConfig, LaunchConfigKind, LaunchConfigOption,
    LaunchConfigSelection, LaunchConfigSource, LaunchProfile, LaunchSelectGroup,
    LaunchSelectOption,
};

pub(crate) const PROFILE_ACCESS_OPTION_ID: &str = "rambledesk.profile.access_mode";
pub(crate) const AGENT_LAUNCH_CONFIG_VERSION: u32 = 1;

pub(crate) fn project_launch_schema(
    profile: &LaunchProfile,
    raw_options: &[Value],
) -> (Vec<LaunchConfigOption>, String) {
    let mut options = raw_options
        .iter()
        .enumerate()
        .map(|(index, option)| project_option(index, option))
        .collect::<Vec<_>>();
    if let Some(option) = profile_access_option(profile) {
        options.push(option);
    }
    let digest = schema_digest(&options);
    (options, digest)
}

pub(crate) fn decode_agent_launch_config(value: &str) -> Option<AgentLaunchConfig> {
    let decoded = serde_json::from_str::<AgentLaunchConfig>(value).ok()?;
    (decoded.version == AGENT_LAUNCH_CONFIG_VERSION).then_some(decoded)
}

pub(crate) fn validate_selections(
    schema_digest_value: &str,
    options: &[LaunchConfigOption],
    values: &[LaunchConfigSelection],
) -> Result<(), String> {
    let actual_digest = schema_digest(options);
    if schema_digest_value != actual_digest {
        return Err(
            "the Agent Launch Schema changed; refresh its options and try again".to_owned(),
        );
    }
    let configurable = options
        .iter()
        .filter(|option| !matches!(option.kind, LaunchConfigKind::Unsupported { .. }))
        .collect::<Vec<_>>();
    if configurable.len() != values.len()
        || configurable
            .iter()
            .zip(values)
            .any(|(option, selection)| option.id != selection.id)
    {
        return Err(
            "Launch config values must follow the complete order advertised by the Agent"
                .to_owned(),
        );
    }
    for (option, selection) in configurable.into_iter().zip(values) {
        match &option.kind {
            LaunchConfigKind::Select { options, .. } => {
                if !options
                    .iter()
                    .any(|candidate| selection.value.as_str() == Some(&candidate.value))
                {
                    return Err(format!(
                        "the Agent no longer offers the selected value for {}",
                        option.id
                    ));
                }
            }
            LaunchConfigKind::Boolean { .. } if !selection.value.is_boolean() => {
                return Err(format!("{} requires a boolean value", option.id));
            }
            LaunchConfigKind::Boolean { .. } => {}
            LaunchConfigKind::Unsupported { .. } => unreachable!("filtered above"),
        }
    }
    Ok(())
}

fn project_option(index: usize, raw: &Value) -> LaunchConfigOption {
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("unsupported-{index}"));
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&id)
        .to_owned();
    let description = optional_string(raw, "description");
    let category = optional_string(raw, "category");
    let raw_type = raw
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let kind = match raw_type.as_str() {
        "select" => project_select(raw).unwrap_or_else(|| LaunchConfigKind::Unsupported {
            raw_type: raw_type.clone(),
            current_value: Value::Null,
            raw: raw.clone(),
        }),
        "boolean" => raw
            .get("currentValue")
            .and_then(Value::as_bool)
            .map(|current_value| LaunchConfigKind::Boolean { current_value })
            .unwrap_or_else(|| LaunchConfigKind::Unsupported {
                raw_type: raw_type.clone(),
                current_value: Value::Null,
                raw: raw.clone(),
            }),
        _ => LaunchConfigKind::Unsupported {
            raw_type,
            current_value: Value::Null,
            raw: raw.clone(),
        },
    };
    LaunchConfigOption {
        id,
        name,
        description,
        category,
        source: LaunchConfigSource::Agent,
        kind,
    }
}

fn project_select(raw: &Value) -> Option<LaunchConfigKind> {
    let current_value = raw.get("currentValue")?.as_str()?.to_owned();
    let mut options = raw
        .get("options")?
        .as_array()?
        .iter()
        .filter_map(project_select_option)
        .collect::<Vec<_>>();
    if options.is_empty() {
        return None;
    }
    let mut groups = Vec::new();
    for (index, group) in raw
        .get("optionGroups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let group_name = optional_string(group, "name")
            .or_else(|| optional_string(group, "group"))
            .unwrap_or_else(|| "Other".to_owned());
        let grouped_values = group
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|option| option.get("value").and_then(Value::as_str))
            .collect::<Vec<_>>();
        for grouped_value in &grouped_values {
            if let Some(option) = options
                .iter_mut()
                .find(|option| option.value == *grouped_value)
            {
                option.group = Some(group_name.clone());
            }
        }
        let grouped_options = options
            .iter()
            .filter(|option| grouped_values.contains(&option.value.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !grouped_options.is_empty() {
            groups.push(LaunchSelectGroup {
                id: optional_string(group, "group")
                    .or_else(|| optional_string(group, "id"))
                    .unwrap_or_else(|| format!("group-{index}")),
                name: group_name,
                options: grouped_options,
            });
        }
    }
    Some(LaunchConfigKind::Select {
        current_value,
        options,
        groups,
    })
}

fn project_select_option(raw: &Value) -> Option<LaunchSelectOption> {
    let value = raw.get("value")?.as_str()?.to_owned();
    Some(LaunchSelectOption {
        name: optional_string(raw, "name").unwrap_or_else(|| value.clone()),
        description: optional_string(raw, "description"),
        value,
        group: None,
    })
}

fn profile_access_option(profile: &LaunchProfile) -> Option<LaunchConfigOption> {
    let policy = &profile.configuration.access_mode;
    let values = match policy.transport {
        AccessModeTransport::ConfigOption => return None,
        AccessModeTransport::ImplicitWorkspaceWrite => {
            vec![profile_access_value("workspace_write", "Workspace write")]
        }
        AccessModeTransport::ProcessArguments => [
            (!policy.read_only.is_empty()).then(|| profile_access_value("read_only", "Read only")),
            (!policy.workspace_write.is_empty())
                .then(|| profile_access_value("workspace_write", "Workspace write")),
            (!policy.yolo.is_empty()).then(|| profile_access_value("yolo", "YOLO")),
        ]
        .into_iter()
        .flatten()
        .collect(),
    };
    let current_value = values
        .iter()
        .find(|option| option.value == "workspace_write")
        .or_else(|| values.first())?
        .value
        .clone();
    Some(LaunchConfigOption {
        id: PROFILE_ACCESS_OPTION_ID.to_owned(),
        name: "Access permission".to_owned(),
        description: Some("Controls process-level access for this Agent profile.".to_owned()),
        category: Some("permissions".to_owned()),
        source: LaunchConfigSource::Profile,
        kind: LaunchConfigKind::Select {
            current_value,
            options: values,
            groups: Vec::new(),
        },
    })
}

fn profile_access_value(value: &str, name: &str) -> LaunchSelectOption {
    LaunchSelectOption {
        value: value.to_owned(),
        name: name.to_owned(),
        description: None,
        group: None,
    }
}

fn optional_string(raw: &Value, field: &str) -> Option<String> {
    raw.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn schema_digest(options: &[LaunchConfigOption]) -> String {
    let mut canonical = serde_json::to_value(options).expect("Launch Config DTO is serializable");
    if let Some(options) = canonical.as_array_mut() {
        for option in options {
            if let Some(object) = option.as_object_mut() {
                object.remove("currentValue");
                if object.get("kind").and_then(Value::as_str) == Some("unsupported")
                    && let Some(raw) = object.get_mut("raw").and_then(Value::as_object_mut)
                {
                    raw.remove("currentValue");
                    raw.remove("_rambledeskMutation");
                }
                remove_internal_keys(object);
            }
        }
    }
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&canonical).expect("canonical Launch Schema serializes")
        ))
    )
}

fn remove_internal_keys(object: &mut Map<String, Value>) {
    object.remove("_rambledeskMutation");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn digest_ignores_current_values_but_not_choices() {
        let profile = LaunchProfile::codex_npx();
        let first = vec![json!({
            "id":"model", "name":"Model", "type":"select", "currentValue":"a",
            "options":[{"value":"a","name":"A"},{"value":"b","name":"B"}]
        })];
        let current_changed = vec![json!({
            "id":"model", "name":"Model", "type":"select", "currentValue":"b",
            "options":[{"value":"a","name":"A"},{"value":"b","name":"B"}]
        })];
        let choices_changed = vec![json!({
            "id":"model", "name":"Model", "type":"select", "currentValue":"a",
            "options":[{"value":"a","name":"A"}]
        })];
        let (_, first_digest) = project_launch_schema(&profile, &first);
        let (_, current_digest) = project_launch_schema(&profile, &current_changed);
        let (_, changed_digest) = project_launch_schema(&profile, &choices_changed);
        assert_eq!(first_digest, current_digest);
        assert_ne!(first_digest, changed_digest);
    }

    #[test]
    fn select_schema_serializes_the_frontend_wire_shape_with_groups() {
        let profile = LaunchProfile::codex_npx();
        let raw = vec![json!({
            "id":"model", "name":"Model", "type":"select", "currentValue":"b",
            "options":[{"value":"a","name":"A"},{"value":"b","name":"B"}],
            "optionGroups":[{
                "group":"recommended", "name":"Recommended",
                "options":[{"value":"b","name":"B"}]
            }]
        })];
        let (schema, _) = project_launch_schema(&profile, &raw);

        let wire = serde_json::to_value(&schema[0]).expect("serialize Launch option");
        assert_eq!(wire["kind"], "select");
        assert!(wire.get("type").is_none());
        assert_eq!(wire["currentValue"], "b");
        assert_eq!(wire["options"][1]["group"], "Recommended");
        assert_eq!(wire["groups"][0]["name"], "Recommended");
        assert_eq!(wire["groups"][0]["options"][0]["value"], "b");
    }

    #[test]
    fn unsupported_option_current_values_do_not_make_the_schema_digest_stale() {
        let profile = LaunchProfile::codex_npx();
        let first = vec![json!({
            "id":"temperature", "name":"Temperature", "type":"number", "currentValue":0.2
        })];
        let current_changed = vec![json!({
            "id":"temperature", "name":"Temperature", "type":"number", "currentValue":0.8
        })];

        let (_, first_digest) = project_launch_schema(&profile, &first);
        let (_, changed_digest) = project_launch_schema(&profile, &current_changed);

        assert_eq!(first_digest, changed_digest);
    }
}
