//! First-party, strongly typed workbench definitions generated from one registry.
#[macro_use]
mod framework;
mod catalog;
mod draft;
mod state;
#[cfg(test)]
mod terminal_tests;
#[cfg(test)]
mod tests;
mod validation;
#[cfg(test)]
mod web_review_tests;
use crate::{ActionInput, ApplicationError};
pub use catalog::*;
pub use draft::*;
pub use framework::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use state::*;
use ts_rs::TS;
include!("workbenches/registry.rs");
pub use media_review::validate_media_review_material;
pub use visual_feedback::{validate_visual_feedback_material, visual_feedback_image_dimensions};
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
    let data =
        WorkbenchKind::resolve(&kind, version).and_then(|kind| kind.decode_data(raw.clone()));
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
        let kind = validate_workbench(&input).ok().map(|valid| valid.kind());
        let result = wire.result.map(|raw| {
            kind.and_then(|kind| kind.decode_result(raw.clone()))
                .unwrap_or(WorkbenchResult::Unknown(raw))
        });
        Ok(Self { input, result })
    }
}
