use super::*;
pub fn validate_workbench_draft(
    spec: &WorkbenchSpec,
    state: Option<&WorkbenchState>,
) -> Result<(), ApplicationError> {
    if !validate_workbench(spec)?.draft_valid(state) {
        return Err(ApplicationError::invalid_argument(
            "Invalid workbench draft state.",
        ));
    }
    Ok(())
}
pub fn workbench_result(
    spec: &WorkbenchSpec,
    state: Option<&WorkbenchState>,
) -> Option<WorkbenchResult> {
    validate_workbench(spec).ok()?.result(state)
}
pub fn workbench_result_complete(package: &WorkbenchPackage) -> bool {
    validate_workbench(&package.input)
        .is_ok_and(|validated| validated.complete(package.result.as_ref()))
}
