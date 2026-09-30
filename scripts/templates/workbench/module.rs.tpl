use super::*;
pub(super) fn draft_valid(_: &__Pascal__Data, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::__Pascal__(state)) => {
            state.score.is_none_or(|score| (1..=5).contains(&score))
                && !state.note.contains('\0')
                && state.note.chars().count() <= 4000
        }
        _ => false,
    }
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::__Pascal__(_)))
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct __Pascal__Data {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 120000))]
    pub material: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct __Pascal__State {
    #[schemars(range(min = 1, max = 5))]
    pub score: Option<u32>,
    #[schemars(length(max = 4000))]
    pub note: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct __Pascal__Result {
    #[schemars(range(min = 1, max = 5))]
    pub score: u32,
    #[schemars(length(max = 4000))]
    pub note: String,
}
pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "__snake__",
            version: 1,
            name: "__Title__",
            purpose: "Rate prepared material and leave an optional opinion.",
            returns: "Score from 1 to 5 and optional opinion text",
            interaction: "__snake__",
        },
        describe,
    );
    definition.strict_state = true;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Ok(WorkbenchDescription {
        summary: definition().summary,
        input_schema: schemars::schema_for!(__Pascal__Data),
        result_schema: schemars::schema_for!(__Pascal__Result),
        example: WorkbenchSpec {
            kind: "__snake__".into(),
            version: 1,
            data: WorkbenchData::__Pascal__(__Pascal__Data {
                title: "Review this proposal".into(),
                material: "Assess whether this proposal is ready to use.".into(),
            }),
        },
        instructions: "Choose a score from 1 to 5. Opinion text is optional. A score is required independently of shared feedback notes.",
    })
}
pub(super) fn validate(data: &__Pascal__Data) -> Result<(), ApplicationError> {
    super::validation::text("__snake__.title", &data.title, 200)?;
    super::validation::text("__snake__.material", &data.material, 120000)
}
pub(super) fn result(
    _: &__Pascal__Data,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::__Pascal__(state)) = state else {
        return None;
    };
    let score = state.score?;
    if !(1..=5).contains(&score) || state.note.contains('\0') || state.note.chars().count() > 4000 {
        return None;
    }
    Some(WorkbenchResult::__Pascal__(__Pascal__Result {
        score,
        note: state.note.clone(),
    }))
}
pub(super) fn has_input(data: &__Pascal__Data, state: Option<&WorkbenchState>) -> bool {
    result(data, state).is_some()
        || matches!(state, Some(WorkbenchState::__Pascal__(state)) if !state.note.trim().is_empty())
}
pub(super) fn complete(_: &__Pascal__Data, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::__Pascal__(_)))
}
pub(super) fn legacy_actions(_: &__Pascal__Data) -> Vec<ActionInput> {
    Vec::new()
}
