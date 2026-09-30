//! First-party workbench contracts. Discovery is bounded; schemas are loaded on demand.
mod catalog;
mod document_review;
mod draft;
mod state;
#[cfg(test)]
mod tests;
mod validation;
mod web_review;
#[cfg(test)]
mod web_review_tests;
pub use document_review::*;
pub use draft::*;
pub use state::*;
pub use validation::*;
pub use web_review::*;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ActionInput, ApplicationError};
pub use catalog::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkbenchSpec {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default = "version_one")]
    pub version: u32,
    /// Get this type's data schema with describe_workbench before creating a request.
    pub data: WorkbenchData,
}

impl<'de> Deserialize<'de> for WorkbenchSpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireSpec {
            #[serde(rename = "type")]
            kind: String,
            #[serde(default = "version_one")]
            version: u32,
            data: serde_value::Value,
        }
        let wire = WireSpec::deserialize(deserializer)?;
        Ok(decode_workbench_spec(wire.kind, wire.version, wire.data))
    }
}

fn decode_workbench_spec(kind: String, version: u32, raw: serde_value::Value) -> WorkbenchSpec {
    // Version selection precedes deserialization: a future contract must not
    // lose nested fields or acquire today's defaults just because it happens
    // to resemble a current data shape.
    let data = match WorkbenchKind::resolve(&kind, version) {
        Some(WorkbenchKind::Ramble) => raw
            .clone()
            .deserialize_into()
            .ok()
            .map(WorkbenchData::Ramble),
        Some(WorkbenchKind::Questions) => raw
            .clone()
            .deserialize_into()
            .ok()
            .map(WorkbenchData::Questions),
        Some(WorkbenchKind::SingleChoice) => raw
            .clone()
            .deserialize_into()
            .ok()
            .map(WorkbenchData::SingleChoice),
        Some(WorkbenchKind::DocumentReview) => raw
            .clone()
            .deserialize_into()
            .ok()
            .map(WorkbenchData::DocumentReview),
        Some(WorkbenchKind::WebReview) => raw
            .clone()
            .deserialize_into()
            .ok()
            .map(WorkbenchData::WebReview),
        None => None,
    };
    let mut spec = WorkbenchSpec {
        kind,
        version,
        data: data.unwrap_or_else(|| WorkbenchData::Unknown(raw.clone())),
    };
    // Invalid known input is preserved for read-only historical inspection,
    // rather than partially normalized. Request validation still rejects it.
    if validate_workbench(&spec).is_err() {
        spec.data = WorkbenchData::Unknown(raw);
    }
    spec
}

fn version_one() -> u32 {
    1
}

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
    DocumentReview(DocumentReviewData),
    WebReview(WebReviewData),
    /// Preserve future request data when reading a library created by a newer app.
    /// Validation rejects this variant for creation and editing.
    Unknown(#[ts(type = "Record<string, unknown>")] serde_value::Value),
}

impl JsonSchema for WorkbenchData {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WorkbenchData".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"object", "description":"Type-specific data. Call describe_workbench to obtain the selected type's input schema and example."})
    }
}

/// Compatibility projection for the three original workbench contracts.
/// These values are persisted and hashed by existing requests: changing this
/// projection requires an explicit identity migration, even if the UI changes.
pub fn workbench_actions(spec: &WorkbenchSpec) -> Result<Vec<ActionInput>, ApplicationError> {
    Ok(validate_workbench(spec)?.legacy_capture_actions())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AnswerStatus {
    Answered,
    Unanswered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged, deny_unknown_fields)]
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
    DocumentReview(DocumentReviewResult),
    WebReview(WebReviewResult),
    /// Opaque published results from future contracts are readable, never used
    /// to authorize a submission under a contract this app does not understand.
    Unknown(
        #[ts(type = "Record<string, unknown>")]
        #[schemars(schema_with = "opaque_workbench_schema")]
        serde_value::Value,
    ),
}

fn opaque_workbench_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type":"object"})
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
pub struct WorkbenchPackage {
    #[serde(flatten)]
    pub input: WorkbenchSpec,
    /// None for cancellation or an unavailable structured document.
    pub result: Option<WorkbenchResult>,
}

impl<'de> Deserialize<'de> for WorkbenchPackage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct WirePackage {
            #[serde(rename = "type")]
            kind: String,
            #[serde(default = "version_one")]
            version: u32,
            data: serde_value::Value,
            result: Option<serde_value::Value>,
        }
        let wire = WirePackage::deserialize(deserializer)?;
        let input = decode_workbench_spec(wire.kind, wire.version, wire.data);
        let supported = validate_workbench(&input).is_ok();
        let result = wire.result.map(|raw| {
            if supported {
                raw.clone()
                    .deserialize_into::<WorkbenchResult>()
                    .unwrap_or(WorkbenchResult::Unknown(raw))
            } else {
                WorkbenchResult::Unknown(raw)
            }
        });
        Ok(Self { input, result })
    }
}
