use super::*;
pub(super) fn draft_valid(data: &QuestionsData, state: Option<&WorkbenchState>) -> bool {
    state.is_none() || result(data, state).is_some()
}
use std::collections::HashSet;
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result,Some(WorkbenchResult::Questions{answers,..}) if !answers.is_empty())
}
use super::validation::{count, id, text};
use schemars::json_schema;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Question {
    #[schemars(regex(pattern = "^[a-z0-9][a-z0-9_-]{0,63}$"))]
    pub id: String,
    #[schemars(length(min = 1, max = 2000))]
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(length(max = 40))]
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
    #[schemars(length(max = 2000))]
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
pub struct QuestionAnswer {
    pub id: String,
    pub value: String,
    pub label: String,
    #[serde(rename = "wasCustom")]
    pub was_custom: bool,
    /// One-based option index. Absent for custom input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub index: Option<u32>,
}

pub(super) fn definition() -> WorkbenchDefinition {
    let (name, purpose, returns, interaction) = (
        "Questions / 逐项问答",
        "Ask one or more questions with options and optional custom answers. Use one question with allowOther:false for a single choice. 单选、多题问答、需求澄清。",
        "Answers associated with question id: value, label, wasCustom, index",
        "questionnaire",
    );
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "questions",
            version: 1,
            name,
            purpose,
            returns,
            interaction,
        },
        describe,
    );
    definition.legacy_identity = true;
    definition.validate_saved_draft = false;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    let (input_schema, result_schema, data, instructions) = (
        schemars::schema_for!(QuestionsData),
        json_schema!({"type":"object","properties":{"answers":{"type":"array","items": schemars::schema_for!(QuestionAnswer)},"cancelled":{"const":false}},"required":["answers","cancelled"]}),
        WorkbenchData::Questions(QuestionsData {
            questions: vec![Question {
                id: "audience".into(),
                prompt: "Who is the main audience?".into(),
                label: Some("Audience".into()),
                allow_other: false,
                options: vec![
                    QuestionOption {
                        value: "individuals".into(),
                        label: "Individuals".into(),
                        description: Some("Optimize for one person's workflow.".into()),
                    },
                    QuestionOption {
                        value: "teams".into(),
                        label: "Teams".into(),
                        description: Some("Prioritize collaboration.".into()),
                    },
                ],
            }],
        }),
        "Use one question with allowOther:false for a single-choice decision, or multiple questions for a questionnaire. Choose one option per question; allowOther:true also permits a custom answer. Read the selected option value from result.answers for that question id. Review all answers before submitting. Every question needs an answer; cancelling produces no result. Answers are independent of optional feedback notes.",
    );
    Ok(WorkbenchDescription {
        summary: definition().summary,
        example: WorkbenchSpec {
            kind: "questions".into(),
            version: 1,
            data,
        },
        input_schema,
        result_schema,
        instructions,
    })
}
pub(super) fn validate(data: &QuestionsData) -> Result<(), ApplicationError> {
    let mut ids = std::collections::HashSet::new();

    count("questions", data.questions.len(), 1, 20)?;
    for question in &data.questions {
        id("questions.id", &question.id, &mut ids)?;
        text("questions.prompt", &question.prompt, 2000)?;
        if let Some(label) = &question.label {
            crate::feedback::validate_text("questions.label", label, 0, 40)?;
        }
        count("questions.options", question.options.len(), 2, 6)?;
        let mut values = HashSet::new();
        for option in &question.options {
            text("questions.options.value", &option.value, 64)?;
            if !values.insert(&option.value) {
                return Err(ApplicationError::invalid_argument(
                    "Question option values must be unique",
                ));
            }
            text("questions.options.label", &option.label, 2000)?;
            if let Some(description) = &option.description {
                crate::feedback::validate_text(
                    "questions.options.description",
                    description,
                    0,
                    2000,
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn result(
    data: &QuestionsData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::Questions { answers }) = state else {
        return None;
    };

    if answers.len() > data.questions.len() {
        return None;
    }
    let mut seen = std::collections::HashSet::new();
    let mut resolved = Vec::new();
    for answer in answers {
        if !seen.insert(&answer.id) {
            return None;
        }
        let question = data
            .questions
            .iter()
            .find(|question| question.id == answer.id)?;
        if answer.was_custom {
            if !question.allow_other
                || answer.value.chars().count() > 4000
                || answer.value.contains('\0')
            {
                return None;
            }
            if !answer.value.trim().is_empty() {
                let value = answer.value.trim().to_owned();
                resolved.push(QuestionAnswer {
                    id: answer.id.clone(),
                    label: value.clone(),
                    value,
                    was_custom: true,
                    index: None,
                });
            }
        } else {
            let (index, option) = question
                .options
                .iter()
                .enumerate()
                .find(|(_, option)| option.value == answer.value)?;
            resolved.push(QuestionAnswer {
                id: answer.id.clone(),
                value: option.value.clone(),
                label: option.label.clone(),
                was_custom: false,
                index: Some(index as u32 + 1),
            });
        }
    }
    resolved.sort_by_key(|answer| {
        data.questions
            .iter()
            .position(|question| question.id == answer.id)
    });
    Some(WorkbenchResult::Questions {
        answers: resolved,
        cancelled: false,
    })
}
pub(super) fn has_input(data: &QuestionsData, state: Option<&WorkbenchState>) -> bool {
    matches!(result(data,state),Some(WorkbenchResult::Questions{answers,..}) if !answers.is_empty())
}
pub(super) fn complete(data: &QuestionsData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result,Some(WorkbenchResult::Questions{answers,..}) if answers.len()==data.questions.len())
}
pub(super) fn legacy_actions(data: &QuestionsData) -> Vec<ActionInput> {
    data.questions
        .iter()
        .map(|q| ActionInput {
            id: q.id.clone(),
            instruction: q.prompt.clone(),
        })
        .collect()
}
