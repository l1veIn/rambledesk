use serde::Serialize;

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

pub const SCREEN_CAPTURE_RESTART_REQUIRED: &str = "SCREEN_CAPTURE_PERMISSION_RESTART_REQUIRED";

#[cfg(target_os = "macos")]
static SCREEN_CAPTURE_RESTART_PENDING: AtomicBool = AtomicBool::new(false);

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
fn screen_capture_view(has_permission: bool, restart_pending: bool) -> (MacPermissionStatus, bool) {
    if has_permission {
        (MacPermissionStatus::Granted, false)
    } else if restart_pending {
        (MacPermissionStatus::Granted, true)
    } else {
        (MacPermissionStatus::NotDetermined, false)
    }
}

#[cfg(target_os = "macos")]
fn current_screen_capture_access() -> (MacPermissionStatus, bool) {
    let has_permission = scap::has_permission();
    if has_permission {
        SCREEN_CAPTURE_RESTART_PENDING.store(false, Ordering::Relaxed);
    }
    screen_capture_view(
        has_permission,
        SCREEN_CAPTURE_RESTART_PENDING.load(Ordering::Relaxed),
    )
}

#[cfg(target_os = "macos")]
fn request_screen_capture_access() -> (MacPermissionStatus, bool) {
    let current = current_screen_capture_access();
    if current.0 == MacPermissionStatus::Granted {
        return current;
    }

    let granted = scap::request_permission();
    let current = current_screen_capture_access();
    if current.0 == MacPermissionStatus::Granted {
        return current;
    }
    if granted {
        SCREEN_CAPTURE_RESTART_PENDING.store(true, Ordering::Relaxed);
        (MacPermissionStatus::Granted, true)
    } else {
        current
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
        let (screen_capture_status, screen_capture_restart_required) =
            current_screen_capture_access();
        vec![
            MacPermissionView::new(
                "screen_capture",
                screen_capture_status,
                screen_capture_restart_required,
            ),
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
    fn pending_screen_capture_grant_stays_granted_until_restart() {
        assert_eq!(
            screen_capture_view(false, true),
            (MacPermissionStatus::Granted, true)
        );
        assert_eq!(
            screen_capture_view(true, true),
            (MacPermissionStatus::Granted, false)
        );
        assert_eq!(
            screen_capture_view(false, false),
            (MacPermissionStatus::NotDetermined, false)
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
