use serde::Deserialize;
use tauri_plugin_opener::OpenerExt;

use crate::WorkbenchState;

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAttachmentInput {
    request_id: String,
    attachment_id: String,
    /// "workspace" (default) resolves the mutable draft attachment; "request"
    /// resolves the immutable request attachment.
    #[serde(default)]
    kind: Option<String>,
}

#[tauri::command]
pub async fn open_feedback_attachment(
    input: OpenAttachmentInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<String, String> {
    let application = state.application.clone();
    let path = match input.kind.as_deref() {
        None | Some("workspace") => application
            .resolve_feedback_attachment_path(input.request_id.clone(), input.attachment_id.clone())
            .await
            .map_err(|error| error.to_string())?,
        Some("request") => application
            .resolve_request_attachment_path(input.request_id.clone(), input.attachment_id.clone())
            .await
            .map_err(|error| error.to_string())?,
        Some(other) => return Err(format!("unknown attachment kind: {other}")),
    };
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|error| format!("无法用系统默认应用打开 {path}：{error}"))?;
    Ok(path)
}
