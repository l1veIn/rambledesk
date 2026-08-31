use std::path::Path;

use rambledesk_acp_client::LaunchConfigKind;

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
    if input.agent_id != preflight.agent_id || input.schema_digest != preflight.schema_digest {
        return Err(AcpWorkbenchError::new(
            "ACP_LAUNCH_SCHEMA_STALE",
            "the Agent Launch Schema changed; refresh its options and try again",
            false,
        ));
    }
    let configurable = preflight
        .config_options
        .iter()
        .filter(|option| !matches!(option.kind, LaunchConfigKind::Unsupported { .. }))
        .collect::<Vec<_>>();
    if configurable.len() != input.config_values.len()
        || configurable
            .iter()
            .zip(&input.config_values)
            .any(|(option, selection)| option.id != selection.id)
    {
        return Err(AcpWorkbenchError::new(
            "ACP_INVALID_CONFIG_SELECTION",
            "Launch config values must follow the complete order advertised by the Agent",
            false,
        ));
    }
    for (option, selection) in configurable.into_iter().zip(&input.config_values) {
        let valid = match &option.kind {
            LaunchConfigKind::Select { options, .. } => options
                .iter()
                .any(|candidate| selection.value.as_str() == Some(&candidate.value)),
            LaunchConfigKind::Boolean { .. } => selection.value.is_boolean(),
            LaunchConfigKind::Unsupported { .. } => unreachable!("filtered above"),
        };
        if !valid {
            return Err(AcpWorkbenchError::new(
                "ACP_INVALID_CONFIG_SELECTION",
                format!(
                    "the selected value for {} was not returned by the Agent Launch Preflight",
                    option.id
                ),
                false,
            ));
        }
    }
    Ok(())
}
