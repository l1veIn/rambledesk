use serde::Serialize;

pub const SCREEN_CAPTURE_RESTART_REQUIRED: &str = "SCREEN_CAPTURE_PERMISSION_RESTART_REQUIRED";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[serde(rename_all = "snake_case")]
pub enum MacPermissionStatus {
    Granted,
    Denied,
    NotDetermined,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct MacPermissionView {
    pub id: String,
    pub status: MacPermissionStatus,
    pub restart_required: bool,
}

impl MacPermissionView {
    fn new(id: &str, status: MacPermissionStatus, restart_required: bool) -> Self {
        Self {
            id: id.to_owned(),
            status,
            restart_required,
        }
    }
}

#[cfg(target_os = "macos")]
fn screen_capture_status() -> MacPermissionStatus {
    if scap::has_permission() {
        MacPermissionStatus::Granted
    } else {
        MacPermissionStatus::NotDetermined
    }
}

#[cfg(target_os = "macos")]
fn request_screen_capture_access() -> (MacPermissionStatus, bool) {
    if scap::has_permission() {
        return (MacPermissionStatus::Granted, false);
    }
    let granted = scap::request_permission();
    if scap::has_permission() {
        return (MacPermissionStatus::Granted, false);
    }
    if granted {
        (MacPermissionStatus::Granted, true)
    } else {
        (screen_capture_status(), false)
    }
}

pub fn require_screen_capture_access() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let (status, restart_required) = request_screen_capture_access();
        if restart_required {
            return Err(SCREEN_CAPTURE_RESTART_REQUIRED.to_owned());
        }
        if status == MacPermissionStatus::Granted {
            return Ok(());
        }
        Err(
            "RambleDesk 需要“屏幕与系统音频录制”权限。请点“去授权”并允许系统弹窗，然后重启应用再截图。"
                .to_owned(),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn microphone_status() -> MacPermissionStatus {
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    let Some(media_type) = (unsafe { AVMediaTypeAudio }) else {
        return MacPermissionStatus::Unknown;
    };
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };
    if status == AVAuthorizationStatus::Authorized {
        MacPermissionStatus::Granted
    } else if status == AVAuthorizationStatus::NotDetermined {
        MacPermissionStatus::NotDetermined
    } else if status == AVAuthorizationStatus::Denied || status == AVAuthorizationStatus::Restricted
    {
        MacPermissionStatus::Denied
    } else {
        MacPermissionStatus::Unknown
    }
}

#[cfg(target_os = "macos")]
fn request_microphone_access() -> MacPermissionStatus {
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeAudio};

    let Some(media_type) = (unsafe { AVMediaTypeAudio }) else {
        return MacPermissionStatus::Unknown;
    };
    let (sender, receiver) = mpsc::channel::<bool>();
    let block = RcBlock::new(move |granted: Bool| {
        let _ = sender.send(granted.as_bool());
    });
    unsafe {
        AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &block);
    }
    let granted = receiver
        .recv_timeout(Duration::from_secs(120))
        .unwrap_or(false);
    if granted {
        MacPermissionStatus::Granted
    } else {
        microphone_status()
    }
}

#[tauri::command]
pub fn list_macos_permissions() -> Vec<MacPermissionView> {
    #[cfg(target_os = "macos")]
    {
        vec![
            MacPermissionView::new("screen_capture", screen_capture_status(), false),
            MacPermissionView::new("microphone", microphone_status(), false),
        ]
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[tauri::command]
pub async fn request_macos_permission(permission: String) -> Result<MacPermissionView, String> {
    let id = permission.clone();
    let (status, restart_required) = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match id.as_str() {
                "screen_capture" => Ok(request_screen_capture_access()),
                "microphone" => Ok((request_microphone_access(), false)),
                other => Err(format!("未知的 macOS 权限：{other}")),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = id;
            Err("此功能仅在 macOS 上可用".to_owned())
        }
    })
    .await
    .map_err(|error| format!("权限请求任务异常退出：{error}"))??;
    crate::diagnostics::record_event(
        "macos_permission_requested",
        Some(&permission),
        None,
        Some(if status == MacPermissionStatus::Granted {
            "granted"
        } else {
            "not_granted"
        }),
        None,
        None,
    );
    Ok(MacPermissionView::new(
        &permission,
        status,
        restart_required,
    ))
}

#[tauri::command]
pub fn open_macos_privacy_settings(permission: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let pane = match permission.as_str() {
            "screen_capture" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            "microphone" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            other => return Err(format!("未知的 macOS 权限：{other}")),
        };
        std::process::Command::new("open")
            .arg(pane)
            .status()
            .map_err(|error| format!("无法打开系统设置：{error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = permission;
        Err("此功能仅在 macOS 上可用".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_required_code_is_stable() {
        assert_eq!(
            SCREEN_CAPTURE_RESTART_REQUIRED,
            "SCREEN_CAPTURE_PERMISSION_RESTART_REQUIRED"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn lists_expected_macos_permissions() {
        let permissions = list_macos_permissions();
        let ids: Vec<&str> = permissions
            .iter()
            .map(|permission| permission.id.as_str())
            .collect();
        assert_eq!(ids, ["screen_capture", "microphone"]);
        assert!(
            permissions
                .iter()
                .all(|permission| !permission.restart_required)
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn require_screen_capture_is_a_no_op_off_macos() {
        assert!(require_screen_capture_access().is_ok());
    }
}
