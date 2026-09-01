use std::path::{Path, PathBuf};

use rambledesk_core::GetFeedbackInput;
use serde::Deserialize;
use tauri_plugin_opener::OpenerExt;

use crate::WorkbenchState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenAttachmentInput {
    request_id: String,
    attachment_id: String,
    /// "workspace" (default) resolves the mutable draft attachment; "request"
    /// resolves the immutable request attachment.
    #[serde(default)]
    kind: Option<String>,
}

async fn resolve_attachment_path(
    input: OpenAttachmentInput,
    state: &WorkbenchState,
) -> Result<String, String> {
    let application = state.application.clone();
    Ok(match input.kind.as_deref() {
        None | Some("workspace") => application
            .resolve_feedback_attachment_path(input.request_id.clone(), input.attachment_id.clone())
            .await
            .map_err(|error| error.to_string())?,
        Some("request") => application
            .resolve_request_attachment_path(input.request_id.clone(), input.attachment_id.clone())
            .await
            .map_err(|error| error.to_string())?,
        Some(other) => return Err(format!("unknown attachment kind: {other}")),
    })
}

#[tauri::command]
pub async fn open_feedback_attachment(
    input: OpenAttachmentInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<String, String> {
    let path = resolve_attachment_path(input, state.inner()).await?;
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|error| format!("无法用系统默认应用打开 {path}：{error}"))?;
    Ok(path)
}

#[tauri::command]
pub async fn reveal_feedback_attachment(
    input: OpenAttachmentInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<String, String> {
    let resolved = resolve_attachment_path(input, state.inner()).await?;
    let path = normalized_existing_path(&resolved)?;
    if app.opener().reveal_item_in_dir(&path).is_err() {
        reveal_with_system_command(&path)
            .map_err(|error| format!("无法在文件夹中显示 {}：{error}", display_os_path(&path)))?;
    }
    Ok(display_os_path(&path))
}

#[tauri::command]
pub async fn reveal_feedback_package(
    input: GetFeedbackInput,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), String> {
    let request = state
        .application
        .get_feedback(input)
        .await
        .map_err(|error| error.to_string())?;
    let feedback = request
        .feedback
        .ok_or_else(|| "feedback package is not available".to_owned())?;
    let path = normalized_existing_path(&feedback.markdown_path)?;
    if app.opener().reveal_item_in_dir(&path).is_err() {
        reveal_with_system_command(&path)
            .map_err(|error| format!("无法在文件夹中显示 {}：{error}", display_os_path(&path)))?;
    }
    Ok(())
}

pub(crate) fn display_os_path(path: &Path) -> String {
    #[cfg(windows)]
    {
        dunce::simplified(path).to_string_lossy().into_owned()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().into_owned()
    }
}

fn normalized_existing_path(path: &str) -> Result<PathBuf, String> {
    let raw = PathBuf::from(path);
    #[cfg(windows)]
    let path = dunce::simplified(&raw).to_path_buf();
    #[cfg(not(windows))]
    let path = raw;
    if path.exists() {
        Ok(path)
    } else {
        Err(format!("找不到文件：{}", display_os_path(&path)))
    }
}

fn reveal_with_system_command(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .status()
            .map_err(|error| format!("无法调用 Finder：{error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Finder 返回 {status}"))
        }
    }
    #[cfg(target_os = "windows")]
    {
        // explorer.exe returns a non-zero code even when /select succeeds.
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", display_os_path(path)))
            .status();
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let parent = path.parent().unwrap_or(path);
        let status = std::process::Command::new("xdg-open")
            .arg(parent)
            .status()
            .map_err(|error| format!("无法打开文件夹：{error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("xdg-open 返回 {status}"))
        }
    }
}

#[tauri::command]
pub fn reveal_path_in_folder(path: String, app: tauri::AppHandle) -> Result<(), String> {
    let path = normalized_existing_path(&path)?;
    if app.opener().reveal_item_in_dir(&path).is_ok() {
        return Ok(());
    }
    reveal_with_system_command(&path)
        .map_err(|error| format!("无法在文件夹中显示 {}：{error}", display_os_path(&path)))
}

#[cfg(test)]
mod tests {
    use super::{OpenAttachmentInput, display_os_path};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn open_attachment_input_accepts_tauri_camel_case_payload() {
        let input: OpenAttachmentInput = serde_json::from_value(json!({
            "requestId": "request-1",
            "attachmentId": "attachment-1",
            "kind": "request",
        }))
        .expect("deserialize tauri payload");

        assert_eq!(input.request_id, "request-1");
        assert_eq!(input.attachment_id, "attachment-1");
        assert_eq!(input.kind.as_deref(), Some("request"));
    }

    #[test]
    fn display_path_keeps_posix_and_ordinary_windows_paths() {
        assert_eq!(
            display_os_path(Path::new("/Users/a/RambleDesk-diagnostics.zip")),
            "/Users/a/RambleDesk-diagnostics.zip"
        );
        assert_eq!(
            display_os_path(Path::new(r"D:\feedback\report.zip")),
            r"D:\feedback\report.zip"
        );
    }
}
