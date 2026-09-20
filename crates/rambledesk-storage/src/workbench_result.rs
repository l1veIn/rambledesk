use rambledesk_core::*;
use serde_json::Value;

/// Interaction state lives beside `doc`, never in editor nodes or Markdown.
pub fn workbench_package(
    spec: &WorkbenchSpec,
    document_json: Option<&str>,
    submitted: bool,
) -> WorkbenchPackage {
    let envelope = document_json.and_then(|value| serde_json::from_str::<Value>(value).ok());
    let result = if submitted {
        envelope
            .as_ref()
            .filter(|value| value["schemaVersion"] == 2 && value["doc"]["type"] == "doc")
            .and_then(|value| {
                let state = value
                    .get("workbenchState")
                    .cloned()
                    .and_then(|state| serde_json::from_value::<WorkbenchState>(state).ok());
                workbench_result(spec, state.as_ref())
            })
    } else {
        None
    };
    WorkbenchPackage {
        input: spec.clone(),
        result,
    }
}
