//! Grok's advertised sessionConfig extension, adapted from Codeg's ACP mapping.
//! Its `mode` category is reasoning effort; it never grants permission modes.
use super::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

const MODEL: &str = "x.ai/model";
const EFFORT: &str = "x.ai/reasoning_effort";

pub(super) struct Configuration {
    model_efforts: BTreeMap<String, Option<SessionConfigOption>>,
    options: Vec<SessionConfigOption>,
}

fn invalid() -> AcpError {
    AcpError::Protocol("Grok session configuration")
}

fn choices(values: &[Value]) -> Result<Vec<SessionConfigChoice>, AcpError> {
    if values.len() > 1024 {
        return Err(invalid());
    }
    let mut seen = HashSet::new();
    values
        .iter()
        .map(|value| {
            let id = mapping::identifier(
                value
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid)?,
            )?;
            if !seen.insert(id.clone()) {
                return Err(invalid());
            }
            Ok(SessionConfigChoice {
                name: mapping::label(value.get("label").and_then(Value::as_str).unwrap_or(&id))?,
                description: value
                    .get("description")
                    .and_then(Value::as_str)
                    .map(mapping::label)
                    .transpose()?,
                value: id,
                group: None,
            })
        })
        .collect()
}

fn selector(
    id: &str,
    current_value: String,
    options: Vec<SessionConfigChoice>,
) -> SessionConfigOption {
    SessionConfigOption {
        id: id.into(),
        name: if id == MODEL {
            "Model"
        } else {
            "Reasoning effort"
        }
        .into(),
        description: None,
        category: Some(
            if id == MODEL {
                "model"
            } else {
                "thought_level"
            }
            .into(),
        ),
        kind: SessionConfigKind::Select {
            current_value,
            options,
        },
    }
}

fn flat_selector(id: &str, values: Vec<Value>) -> Result<Option<SessionConfigOption>, AcpError> {
    let options = choices(&values)?;
    let Some(first) = options.first() else {
        return Ok(None);
    };
    let selected: Vec<_> = values
        .iter()
        .filter(|value| value.get("selected").and_then(Value::as_bool) == Some(true))
        .collect();
    if selected.len() > 1 {
        return Err(invalid());
    }
    // Absence of a selected value is not evidence of the first option being current.
    let Some(selected) = selected.first() else {
        return Ok(None);
    };
    let current = selected
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(&first.value)
        .to_owned();
    Ok(Some(selector(id, current, options)))
}

impl Configuration {
    pub fn parse(raw: &Value) -> Result<Option<Self>, AcpError> {
        let Some(extension) = raw
            .get("_meta")
            .and_then(|meta| meta.get("x.ai/sessionConfig"))
        else {
            return Ok(None);
        };
        if serde_json::to_vec(extension).map_err(|_| invalid())?.len() > 512 * 1024 {
            return Err(invalid());
        }
        let Some(values) = extension.get("options").and_then(Value::as_array) else {
            return Ok(None);
        };
        if values.len() > 1024 {
            return Err(invalid());
        }
        let model = flat_selector(
            MODEL,
            values
                .iter()
                .filter(|value| value.get("category").and_then(Value::as_str) == Some("model"))
                .cloned()
                .collect(),
        )?;
        let Some(model) = model else {
            return Ok(None);
        };
        let mut options = vec![model];
        let mut model_efforts = BTreeMap::new();
        if let Some(models) = raw
            .get("models")
            .and_then(|models| models.get("availableModels"))
            .and_then(Value::as_array)
        {
            if models.len() > 512
                || serde_json::to_vec(models).map_err(|_| invalid())?.len() > 512 * 1024
            {
                return Err(invalid());
            }
            for model in models {
                let id = mapping::identifier(
                    model
                        .get("modelId")
                        .and_then(Value::as_str)
                        .ok_or_else(invalid)?,
                )?;
                let meta = model.get("_meta");
                let mut effort = None;
                if meta
                    .and_then(|meta| meta.get("supportsReasoningEffort"))
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    let values = meta
                        .and_then(|meta| meta.get("reasoningEfforts"))
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let mut choices = choices(values)?;
                    if let Some(default) = meta
                        .and_then(|meta| meta.get("reasoningEffort"))
                        .and_then(Value::as_str)
                    {
                        let default = mapping::identifier(default)?;
                        if !choices.iter().any(|choice| choice.value == default) {
                            choices.insert(
                                0,
                                SessionConfigChoice {
                                    value: default.clone(),
                                    name: mapping::label(&default)?,
                                    description: None,
                                    group: None,
                                },
                            );
                        }
                        effort = Some(selector(EFFORT, default, choices));
                    }
                }
                if model_efforts.insert(id, effort).is_some() {
                    return Err(invalid());
                }
            }
        }
        if model_efforts.is_empty() {
            if let Some(effort) = flat_selector(
                EFFORT,
                values
                    .iter()
                    .filter(|value| value.get("category").and_then(Value::as_str) == Some("mode"))
                    .cloned()
                    .collect(),
            )? {
                options.push(effort);
            }
        } else if let Some(effort) = current(&options, MODEL)
            .and_then(|model| model_efforts.get(model))
            .and_then(Option::as_ref)
        {
            options.push(effort.clone());
        }
        Ok(Some(Self {
            model_efforts,
            options,
        }))
    }

    pub fn options(&self) -> &[SessionConfigOption] {
        &self.options
    }

    pub fn request(&self, remote: &str, id: &str, value: &SessionConfigValue) -> Option<Request> {
        let SessionConfigValue::Select { value } = value else {
            return None;
        };
        let params = match id {
            MODEL => json!({ "sessionId": remote, "modelId": value }),
            EFFORT => {
                json!({ "sessionId": remote, "modelId": current(&self.options, MODEL)?, "_meta": { "reasoningEffort": value } })
            }
            _ => return None,
        };
        Some(Request {
            method: "session/set_model",
            params,
        })
    }

    pub fn confirmed(&mut self, id: &str, value: &SessionConfigValue) {
        let SessionConfigValue::Select { value } = value else {
            return;
        };
        if let Some(SessionConfigOption {
            kind: SessionConfigKind::Select { current_value, .. },
            ..
        }) = self.options.iter_mut().find(|option| option.id == id)
        {
            *current_value = value.clone();
        }
        if id == MODEL && !self.model_efforts.is_empty() {
            self.options.retain(|option| option.id != EFFORT);
            if let Some(Some(effort)) = self.model_efforts.get(value) {
                self.options.push(effort.clone());
            }
        }
    }
}

fn current<'a>(options: &'a [SessionConfigOption], id: &str) -> Option<&'a str> {
    match &options.iter().find(|option| option.id == id)?.kind {
        SessionConfigKind::Select { current_value, .. } => Some(current_value),
        _ => None,
    }
}
