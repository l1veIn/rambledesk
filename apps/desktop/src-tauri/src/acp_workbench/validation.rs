use std::path::Path;

use super::model::{AcpWorkbenchError, LaunchDraftInput, LaunchPreflight};

pub(super) fn title_from_markdown(markdown: &str) -> String {
    let title = markdown
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("New Ramble")
        .trim_start_matches('#')
        .trim();
    title.chars().take(160).collect()
}

pub(super) fn nonblank_option(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

pub(super) fn require_nonblank(field: &str, value: &str) -> Result<(), AcpWorkbenchError> {
    if value.trim().is_empty() {
        Err(AcpWorkbenchError::new(
            "INVALID_ARGUMENT",
            format!("{field} must not be blank"),
            false,
        ))
    } else {
        Ok(())
    }
}

pub(super) async fn validate_selected_workspace(workspace: &str) -> Result<(), AcpWorkbenchError> {
    if workspace.trim().is_empty() {
        return Err(invalid_workspace(
            "the selected workspace must not be blank",
        ));
    }
    let path = Path::new(workspace);
    if !path.is_absolute() {
        return Err(invalid_workspace(
            "the selected workspace must be an absolute path",
        ));
    }
    let metadata = tokio::fs::metadata(path).await.map_err(|error| {
        invalid_workspace(format!("the selected workspace cannot be opened: {error}"))
    })?;
    if !metadata.is_dir() {
        return Err(invalid_workspace(
            "the selected workspace must be a directory",
        ));
    }
    let _entries = tokio::fs::read_dir(path).await.map_err(|error| {
        invalid_workspace(format!("the selected workspace cannot be read: {error}"))
    })?;
    Ok(())
}

fn invalid_workspace(message: impl Into<String>) -> AcpWorkbenchError {
    AcpWorkbenchError::new("INVALID_WORKSPACE", message, false)
}

pub(super) fn validate_launch_selection(
    input: &LaunchDraftInput,
    preflight: &LaunchPreflight,
) -> Result<(), AcpWorkbenchError> {
    if !input.model.trim().is_empty() && !preflight.models.contains(&input.model) {
        return Err(AcpWorkbenchError::new(
            "ACP_UNSUPPORTED_MODEL",
            "the selected model was not returned by the Agent Launch Preflight",
            false,
        ));
    }
    if !input.reasoning_effort.trim().is_empty()
        && !preflight
            .reasoning_efforts
            .contains(&input.reasoning_effort)
    {
        return Err(AcpWorkbenchError::new(
            "ACP_UNSUPPORTED_REASONING_EFFORT",
            "the selected reasoning effort was not returned by the Agent Launch Preflight",
            false,
        ));
    }
    if !preflight.access_modes.contains(&input.access_mode) {
        return Err(AcpWorkbenchError::new(
            "ACP_UNSUPPORTED_ACCESS_MODE",
            "the selected Access Mode is not supported by this Agent Launch Profile",
            false,
        ));
    }
    Ok(())
}
