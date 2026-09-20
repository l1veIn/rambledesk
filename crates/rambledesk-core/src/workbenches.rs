//! First-party workbench contracts. Discovery is bounded; schemas are loaded on demand.
mod catalog;
mod state;
pub use state::*;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ActionInput, ApplicationError};
pub use catalog::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkbenchSpec {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default = "version_one")]
    pub version: u32,
    /// Get this type's data schema with describe_workbench before creating a request.
    pub data: WorkbenchData,
}

fn version_one() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RambleData {
    #[schemars(length(min = 1, max = 20))]
    pub actions: Vec<ActionInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Question {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 2000))]
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    #[schemars(length(min = 2, max = 6))]
    pub options: Vec<QuestionOption>,
    #[serde(default = "allow_other", rename = "allowOther")]
    pub allow_other: bool,
}

fn allow_other() -> bool {
    true
}

/// Field names follow Pi's questionnaire extension contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QuestionOption {
    #[schemars(length(min = 1, max = 64))]
    pub value: String,
    #[schemars(length(min = 1, max = 2000))]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QuestionsData {
    #[schemars(length(min = 1, max = 20))]
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 2000))]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SingleChoiceData {
    #[schemars(length(min = 1, max = 2000))]
    pub prompt: String,
    #[schemars(length(min = 2, max = 20))]
    pub options: Vec<ChoiceOption>,
}

/// Typed domain data with an intentionally small discovery-first tool schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum WorkbenchData {
    Ramble(RambleData),
    Questions(QuestionsData),
    SingleChoice(SingleChoiceData),
}

impl JsonSchema for WorkbenchData {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WorkbenchData".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"object", "description":"Type-specific data. Call describe_workbench to obtain the selected type's input schema and example."})
    }
}

/// Keep legacy actions unchanged; typed data projects into the existing capture targets.
pub fn workbench_actions(spec: &WorkbenchSpec) -> Result<Vec<ActionInput>, ApplicationError> {
    describe_workbench(&DescribeWorkbenchInput {
        kind: spec.kind.clone(),
        version: Some(spec.version),
    })?;
    let actions = match (spec.kind.as_str(), &spec.data) {
        ("ramble", WorkbenchData::Ramble(data)) => data.actions.clone(),
        ("questions", WorkbenchData::Questions(data)) => {
            for question in &data.questions {
                if !(2..=6).contains(&question.options.len()) {
                    return Err(ApplicationError::invalid_argument(
                        "Each question needs 2–6 options",
                    ));
                }
                let mut values = std::collections::HashSet::new();
                if question
                    .label
                    .as_ref()
                    .is_some_and(|label| label.chars().count() > 40 || label.contains('\0'))
                {
                    return Err(ApplicationError::invalid_argument(
                        "Question labels must be at most 40 characters without NUL",
                    ));
                }
                for option in &question.options {
                    if option.value.trim().is_empty()
                        || option.value.chars().count() > 64
                        || option.value.contains('\0')
                        || !values.insert(&option.value)
                        || option.label.trim().is_empty()
                        || option.label.chars().count() > 2000
                        || option.label.contains('\0')
                        || option.description.as_ref().is_some_and(|description| {
                            description.chars().count() > 2000 || description.contains('\0')
                        })
                    {
                        return Err(ApplicationError::invalid_argument(
                            "Question options need unique nonempty values and valid labels/descriptions",
                        ));
                    }
                }
            }
            data.questions
                .iter()
                .map(|question| ActionInput {
                    id: question.id.clone(),
                    instruction: question.prompt.clone(),
                })
                .collect()
        }
        ("single_choice", WorkbenchData::SingleChoice(data)) => {
            if data.prompt.trim().is_empty()
                || data.prompt.chars().count() > 2000
                || data.prompt.contains('\0')
            {
                return Err(ApplicationError::invalid_argument(
                    "single_choice.prompt must contain 1–2000 visible characters without NUL",
                ));
            }
            if !(2..=20).contains(&data.options.len()) {
                return Err(ApplicationError::invalid_argument(
                    "single_choice.options must contain 2–20 items",
                ));
            }
            data.options
                .iter()
                .map(|option| ActionInput {
                    id: option.id.clone(),
                    instruction: option.label.clone(),
                })
                .collect()
        }
        _ => {
            return Err(ApplicationError::invalid_argument(
                "workbench.data does not match its type; call describe_workbench for its schema",
            ));
        }
    };
    if actions
        .iter()
        .any(|action| action.instruction.trim().is_empty())
    {
        return Err(ApplicationError::invalid_argument(
            "workbench items must contain visible text",
        ));
    }
    Ok(actions)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AnswerStatus {
    Answered,
    Unanswered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum WorkbenchResult {
    Ramble {
        kind: String,
    },
    Questions {
        answers: Vec<QuestionAnswer>,
        cancelled: bool,
    },
    SingleChoice {
        status: AnswerStatus,
        selected_option_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct WorkbenchPackage {
    #[serde(flatten)]
    pub input: WorkbenchSpec,
    /// None for cancellation or an unavailable structured document.
    pub result: Option<WorkbenchResult>,
}
