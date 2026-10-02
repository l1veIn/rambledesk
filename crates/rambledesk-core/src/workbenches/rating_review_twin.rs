// Test-only result identity regression fixture with the same payload shape as rating_review.
use super::*;
pub(super) fn draft_valid(_: &RatingReviewTwinData, state: Option<&WorkbenchState>) -> bool {
    match state {
        None => true,
        Some(WorkbenchState::RatingReviewTwin(state)) => {
            state.score.is_none_or(|score| (1..=5).contains(&score))
                && !state.note.contains('\0')
                && state.note.chars().count() <= 4000
        }
        _ => false,
    }
}
pub(super) fn result_has_input(result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::RatingReviewTwin(_)))
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RatingReviewTwinData {
    #[schemars(length(min = 1, max = 200))]
    pub title: String,
    #[schemars(length(min = 1, max = 120000))]
    pub material: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RatingReviewTwinState {
    #[schemars(range(min = 1, max = 5))]
    pub score: Option<u32>,
    #[schemars(length(max = 4000))]
    pub note: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RatingReviewTwinResult {
    #[schemars(range(min = 1, max = 5))]
    pub score: u32,
    #[schemars(length(max = 4000))]
    pub note: String,
}
pub(super) fn definition() -> WorkbenchDefinition {
    let mut definition = WorkbenchDefinition::new(
        WorkbenchSummary {
            kind: "rating_review_twin",
            version: 1,
            name: "Rating review / 评分评审",
            purpose: "Development fixture: rate prepared material and leave an optional opinion.",
            returns: "Score from 1 to 5 and optional opinion text",
            interaction: "rating_review_twin",
        },
        describe,
    );
    definition.strict_state = true;
    definition.advertised = false;
    definition
}
fn describe() -> Result<WorkbenchDescription, ApplicationError> {
    Ok(WorkbenchDescription {
        summary: definition().summary,
        input_schema: schemars::schema_for!(RatingReviewTwinData),
        result_schema: schemars::schema_for!(RatingReviewTwinResult),
        example: WorkbenchSpec {
            kind: "rating_review_twin".into(),
            version: 1,
            data: WorkbenchData::RatingReviewTwin(RatingReviewTwinData {
                title: "Review this proposal".into(),
                material: "Assess whether this proposal is ready to use.".into(),
            }),
        },
        instructions: "Choose a score from 1 to 5. Opinion text is optional. A score is required independently of shared feedback notes. Development fixture only.",
    })
}
pub(super) fn validate(data: &RatingReviewTwinData) -> Result<(), ApplicationError> {
    super::validation::text("rating_review_twin.title", &data.title, 200)?;
    super::validation::text("rating_review_twin.material", &data.material, 120000)
}
pub(super) fn result(
    _: &RatingReviewTwinData,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    let Some(WorkbenchState::RatingReviewTwin(state)) = state else {
        return None;
    };
    let score = state.score?;
    if !(1..=5).contains(&score) || state.note.contains('\0') || state.note.chars().count() > 4000 {
        return None;
    }
    Some(WorkbenchResult::RatingReviewTwin(RatingReviewTwinResult {
        score,
        note: state.note.clone(),
    }))
}
pub(super) fn has_input(data: &RatingReviewTwinData, state: Option<&WorkbenchState>) -> bool {
    result(data, state).is_some()
        || matches!(state, Some(WorkbenchState::RatingReviewTwin(state)) if !state.note.trim().is_empty())
}
pub(super) fn complete(_: &RatingReviewTwinData, result: Option<&WorkbenchResult>) -> bool {
    matches!(result, Some(WorkbenchResult::RatingReviewTwin(_)))
}
pub(super) fn legacy_actions(_: &RatingReviewTwinData) -> Vec<ActionInput> {
    Vec::new()
}

#[test]
fn packages_with_identical_result_shapes_preserve_type_and_future_payloads() {
    let rating = super::rating_review::definition();
    let twin = definition();
    for definition in [rating, twin] {
        let spec = (definition.describe)().unwrap().example;
        let raw = serde_value::to_value(RatingReviewResult {
            score: 4,
            note: "same shape".into(),
        })
        .unwrap();
        let result = WorkbenchKind::resolve(&spec.kind, spec.version)
            .unwrap()
            .decode_result(raw)
            .unwrap();
        let package = WorkbenchPackage {
            input: spec,
            result: Some(result),
        };
        let decoded: WorkbenchPackage = serde_value::to_value(&package)
            .unwrap()
            .deserialize_into()
            .unwrap();
        assert_eq!(decoded, package);
        assert!(workbench_result_complete(&decoded));

        let mut future = package;
        future.input.version = 99;
        let raw = serde_value::to_value(&future).unwrap();
        let decoded: WorkbenchPackage = raw.clone().deserialize_into().unwrap();
        assert!(matches!(decoded.input.data, WorkbenchData::Unknown(_)));
        assert!(matches!(decoded.result, Some(WorkbenchResult::Unknown(_))));
        assert_eq!(serde_value::to_value(decoded).unwrap(), raw);
    }
}
