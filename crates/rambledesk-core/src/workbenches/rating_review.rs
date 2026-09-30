use super::*;
pub(super) fn draft_valid(_: &RatingReviewData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::RatingReview(state)) => {
            state.score.is_none_or(|score| (1..=5).contains(&score))
                && !state.note.contains('\0')
                && state.note.chars().count() <= 4000
        }
        _ => false,
    }
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::RatingReview(_)))
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RatingReviewData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 120000))]
    pub material: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RatingReviewState {
    #[schemars(range(min = 1, max = 5))]
    pub score: Option<u32>,
    #[schemars(length(max = 4000))]
    pub note: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RatingReviewResult {
    #[schemars(range(min = 1, max = 5))]
    pub score: u32,
    #[schemars(length(max = 4000))]
    pub note: String,
}
pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "rating_review",
            version: 1,
            name: "Rating review / 评分评审",
            purpose: "Development fixture: rate prepared material and leave an optional opinion.",
            returns: "Score from 1 to 5 and optional opinion text",
            interaction: "rating_review",
        },
        describe,
    );
    definition.strict_state = true;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Ok(WorkbenchDescription {
        summary: definition().summary,
        input_schema: schemars::schema_for!(RatingReviewData),
        result_schema: schemars::schema_for!(RatingReviewResult),
        example: WorkbenchSpec {
            kind: "rating_review".into(),
            version: 1,
            data: WorkbenchData::RatingReview(RatingReviewData {
                title: "Review this proposal".into(),
                material: "Assess whether this proposal is ready to use.".into(),
            }),
        },
        instructions: "Choose a score from 1 to 5. Opinion text is optional. A score is required independently of shared feedback notes. Development fixture only.",
    })
}
pub(super) fn validate(data: &RatingReviewData) -> Result<(), ApplicationError> {
    super::validation::text("rating_review.title", &data.title, 200)?;
    super::validation::text("rating_review.material", &data.material, 120000)
}
pub(super) fn result(
    _: &RatingReviewData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::RatingReview(state)) = state else {
        return None;
    };
    let score = state.score?;
    if !(1..=5).contains(&score) || state.note.contains('\0') || state.note.chars().count() > 4000 {
        return None;
    }
    Some(WorkbenchResult::RatingReview(RatingReviewResult {
        score,
        note: state.note.clone(),
    }))
}
pub(super) fn has_input(data: &RatingReviewData, state: Option<&WorkbenchState>) -> bool {
    result(data, state).is_some()
        || matches!(state, Some(WorkbenchState::RatingReview(state)) if !state.note.trim().is_empty())
}
pub(super) fn complete(_: &RatingReviewData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::RatingReview(_)))
}
pub(super) fn legacy_actions(_: &RatingReviewData) -> Vec<ActionInput> {
    Vec::new()
}
