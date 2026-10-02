use super::*;
pub(super) fn draft_valid(data: &SingleChoiceData, state: Option<&WorkbenchState>) -> bool {
    state.is_none() || result(data, state).is_some()
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(
        result,
        Some(WorkbenchResult::SingleChoice {
            selected_option_id: Some(_),
            ..
        })
    )
}
use super::validation::{count, id, text};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AnswerStatus {
    Answered,
    Unanswered,
}

pub(super) fn definition() -> WorkbenchDefinition {
    let (name, purpose, returns, interaction) = (
        "Single choice / 方案单选",
        "Choose one option and optionally explain the decision. 方案比较、单选、偏好选择。",
        "One selected option id with optional feedback notes",
        "single_selection",
    );
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "single_choice",
            version: 1,
            name,
            purpose,
            returns,
            interaction,
        },
        describe,
    );
    definition.legacy_identity = true;
    definition.advertised = false;
    definition.validate_saved_draft = false;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Err(ApplicationError::invalid_argument(
        "single_choice is a compatibility contract. For new requests, describe questions and use one question with allowOther:false. Existing single_choice requests keep their original input and result contract.",
    ))
}
pub(super) fn validate(data: &SingleChoiceData) -> Result<(), ApplicationError> {
    let mut ids = std::collections::HashSet::new();

    text("single_choice.prompt", &data.prompt, 2000)?;
    count("single_choice.options", data.options.len(), 2, 20)?;
    for option in &data.options {
        id("single_choice.options.id", &option.id, &mut ids)?;
        text("single_choice.options.label", &option.label, 2000)?;
    }
    Ok(())
}

pub(super) fn result(
    data: &SingleChoiceData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::SingleChoice { selected_option_id }) = state else {
        return None;
    };

    if selected_option_id
        .as_ref()
        .is_some_and(|id| !data.options.iter().any(|option| option.id == *id))
    {
        return None;
    }
    Some(WorkbenchResult::SingleChoice {
        status: if selected_option_id.is_some() {
            AnswerStatus::Answered
        } else {
            AnswerStatus::Unanswered
        },
        selected_option_id: selected_option_id.clone(),
    })
}
pub(super) fn has_input(data: &SingleChoiceData, state: Option<&WorkbenchState>) -> bool {
    matches!(
        result(data, state),
        Some(WorkbenchResult::SingleChoice {
            selected_option_id: Some(_),
            ..
        })
    )
}
pub(super) fn complete(_: &SingleChoiceData, result: Option<&WorkbenchResult>) -> bool {
    matches!(
        result,
        Some(WorkbenchResult::SingleChoice {
            status: AnswerStatus::Answered,
            ..
        })
    )
}
pub(super) fn legacy_actions(data: &SingleChoiceData) -> Vec<ActionInput> {
    data.options
        .iter()
        .map(|o| ActionInput {
            id: o.id.clone(),
            instruction: o.label.clone(),
        })
        .collect()
}
