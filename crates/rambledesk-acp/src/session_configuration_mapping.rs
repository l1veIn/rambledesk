use super::*;

const MAX_OPTIONS: usize = 128;
const MAX_CHOICES: usize = 1024;
const MAX_BYTES: usize = 512 * 1024;

fn invalid() -> AcpError {
    AcpError::Protocol("session configuration bounds")
}
pub(super) fn identifier(value: &str) -> Result<String, AcpError> {
    if value.trim().is_empty() || value.len() > 1024 || value.contains('\0') {
        return Err(invalid());
    }
    Ok(value.into())
}
fn label(value: &str) -> Result<String, AcpError> {
    if value.len() > 4096 || value.contains('\0') {
        return Err(invalid());
    }
    Ok(value.into())
}
fn description(value: &Option<String>) -> Result<Option<String>, AcpError> {
    value.as_deref().map(label).transpose()
}

pub(super) fn options(
    options: &[acp::SessionConfigOption],
) -> Result<Vec<SessionConfigOption>, AcpError> {
    if options.len() > MAX_OPTIONS {
        return Err(invalid());
    }
    let mut budget = MAX_BYTES;
    let mut seen = std::collections::HashSet::new();
    options
        .iter()
        .map(|option| {
            let id = identifier(&option.id.to_string())?;
            if !seen.insert(id.clone()) {
                return Err(invalid());
            }
            let category = option
                .category
                .as_ref()
                .map(|category| match category {
                    acp::SessionConfigOptionCategory::Mode => "mode",
                    acp::SessionConfigOptionCategory::Model => "model",
                    acp::SessionConfigOptionCategory::ModelConfig => "model_config",
                    acp::SessionConfigOptionCategory::ThoughtLevel => "thought_level",
                    acp::SessionConfigOptionCategory::Other(value) => value.as_str(),
                    _ => "other",
                })
                .map(label)
                .transpose()?;
            let kind = match &option.kind {
                acp::SessionConfigKind::Boolean(value) => SessionConfigKind::Boolean {
                    current_value: value.current_value,
                },
                acp::SessionConfigKind::Select(select) => {
                    let mut options = vec![];
                    match &select.options {
                        acp::SessionConfigSelectOptions::Ungrouped(choices) => {
                            add_choices(&mut options, choices, None)?
                        }
                        acp::SessionConfigSelectOptions::Grouped(groups) => {
                            if groups.len() > MAX_OPTIONS {
                                return Err(invalid());
                            }
                            for group in groups {
                                add_choices(
                                    &mut options,
                                    &group.options,
                                    Some(label(&group.name)?),
                                )?;
                            }
                        }
                        _ => return Err(invalid()),
                    }
                    SessionConfigKind::Select {
                        current_value: identifier(&select.current_value.to_string())?,
                        options,
                    }
                }
                _ => return Err(invalid()),
            };
            let option = SessionConfigOption {
                id,
                name: label(&option.name)?,
                description: description(&option.description)?,
                category,
                kind,
            };
            let size = serde_json::to_vec(&option).map_err(|_| invalid())?.len();
            budget = budget.checked_sub(size).ok_or_else(invalid)?;
            Ok(option)
        })
        .collect()
}

fn add_choices(
    out: &mut Vec<SessionConfigChoice>,
    choices: &[acp::SessionConfigSelectOption],
    group: Option<String>,
) -> Result<(), AcpError> {
    if out.len() + choices.len() > MAX_CHOICES {
        return Err(invalid());
    }
    for choice in choices {
        let value = identifier(&choice.value.to_string())?;
        if out.iter().any(|existing| existing.value == value) {
            return Err(invalid());
        }
        out.push(SessionConfigChoice {
            value,
            name: label(&choice.name)?,
            description: description(&choice.description)?,
            group: group.clone(),
        });
    }
    Ok(())
}

pub(super) fn modes(modes: acp::SessionModeState) -> Result<SessionModeCatalog, AcpError> {
    if modes.available_modes.len() > MAX_OPTIONS {
        return Err(invalid());
    }
    Ok(SessionModeCatalog {
        current_mode_id: identifier(&modes.current_mode_id.to_string())?,
        available_modes: modes
            .available_modes
            .into_iter()
            .map(|mode| {
                Ok(SessionMode {
                    id: identifier(&mode.id.to_string())?,
                    name: label(&mode.name)?,
                    description: description(&mode.description)?,
                })
            })
            .collect::<Result<_, AcpError>>()?,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LegacyModels {
    current_model_id: String,
    available_models: Vec<LegacyModel>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyModel {
    model_id: String,
    name: String,
    description: Option<String>,
}
pub(super) fn models(models: LegacyModels) -> Result<SessionModelCatalog, AcpError> {
    if models.available_models.len() > 512 {
        return Err(invalid());
    }
    Ok(SessionModelCatalog {
        current_model_id: identifier(&models.current_model_id)?,
        available_models: models
            .available_models
            .into_iter()
            .map(|model| {
                Ok(SessionModel {
                    model_id: identifier(&model.model_id)?,
                    name: label(&model.name)?,
                    description: description(&model.description)?,
                })
            })
            .collect::<Result<_, AcpError>>()?,
    })
}
