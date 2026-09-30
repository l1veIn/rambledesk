use super::*;
pub(super) fn draft_valid(_: &RambleData, _: Option<&WorkbenchState>) -> bool {
    true
}
pub(super) fn result_has_input(_: Option<&WorkbenchResult>) -> bool {
    false
}
use super::validation::{count, id, text};
use schemars::json_schema;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RambleData {
    #[schemars(length(min = 1, max = 20))]
    #[serde(deserialize_with = "deserialize_workbench_actions")]
    pub actions: Vec<ActionInput>,
}

fn deserialize_workbench_actions<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<ActionInput>, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct WorkbenchAction {
        id: String,
        instruction: String,
    }
    Vec::<WorkbenchAction>::deserialize(deserializer).map(|actions| {
        actions
            .into_iter()
            .map(|action| ActionInput {
                id: action.id,
                instruction: action.instruction,
            })
            .collect()
    })
}

pub(super) fn definition() -> WorkbenchDefinition {
    let (name, purpose, returns, interaction) = (
        "Ramble / 自由反馈",
        "Review or experience actions, then give free-form feedback with attachments. 自由体验、评审、语音反馈。",
        "Free-form feedback body and attachments",
        "free_feedback",
    );
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "ramble",
            version: 1,
            name,
            purpose,
            returns,
            interaction,
        },
        describe,
    );
    definition.approval = true;
    definition.legacy_identity = true;
    definition.require_complete = false;
    definition.validate_saved_draft = false;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    let (input_schema, result_schema, data, instructions) = (
        schemars::schema_for!(RambleData),
        json_schema!({"type":"object","properties":{"kind":{"const":"free_feedback"}},"required":["kind"]}),
        WorkbenchData::Ramble(RambleData {
            actions: vec![ActionInput {
                id: "try-it".into(),
                instruction: "Try the new flow and describe your experience.".into(),
            }],
        }),
        "Use the shared feedback editor. Read feedback_package.markdown and attachment paths; result.kind is free_feedback.",
    );
    Ok(WorkbenchDescription {
        summary: definition().summary,
        example: WorkbenchSpec {
            kind: "ramble".into(),
            version: 1,
            data,
        },
        input_schema,
        result_schema,
        instructions,
    })
}
pub(super) fn validate(data: &RambleData) -> Result<(), ApplicationError> {
    let mut ids = std::collections::HashSet::new();

    count("ramble.actions", data.actions.len(), 1, 20)?;
    for action in &data.actions {
        id("ramble.actions.id", &action.id, &mut ids)?;
        text("ramble.actions.instruction", &action.instruction, 2000)?;
    }
    Ok(())
}

pub(super) fn result(_: &RambleData, _: Option<&WorkbenchState>) -> Option<WorkbenchResult> {
    Some(WorkbenchResult::Ramble {
        kind: "free_feedback".into(),
    })
}
pub(super) fn has_input(_: &RambleData, _: Option<&WorkbenchState>) -> bool {
    false
}
pub(super) fn complete(_: &RambleData, _: Option<&WorkbenchResult>) -> bool {
    false
}
pub(super) fn legacy_actions(data: &RambleData) -> Vec<ActionInput> {
    data.actions.clone()
}
