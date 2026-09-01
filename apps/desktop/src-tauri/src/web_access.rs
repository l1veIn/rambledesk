use std::sync::Arc;

use rambledesk_local_server::{
    SpaAsset, SpaAssetSource, WebAccessServerConfig, WebAccessServerHandle,
    WebSessionAuthenticator, start_web_access_server,
};
use serde::Serialize;
use tauri::AppHandle;

use crate::WorkbenchState;

struct RejectAllWebSessions;

impl WebSessionAuthenticator for RejectAllWebSessions {
    fn authenticate(&self, _session_token: &str) -> bool {
        false
    }
}

struct TauriSpaAssets {
    app: AppHandle,
}

impl SpaAssetSource for TauriSpaAssets {
    fn load(&self, path: &str) -> Option<SpaAsset> {
        self.app
            .asset_resolver()
            .get(path.to_owned())
            .map(|asset| SpaAsset {
                bytes: asset.bytes,
                mime_type: asset.mime_type,
                // The loopback server supplies a Web-specific policy. The
                // embedded Tauri policy contains IPC origins unavailable to a browser.
                content_security_policy: None,
            })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WebAccessStatus {
    running: bool,
    url: Option<String>,
}

fn status(server: Option<&WebAccessServerHandle>) -> WebAccessStatus {
    WebAccessStatus {
        running: server.is_some(),
        url: server.map(|server| server.origin().to_owned()),
    }
}

#[tauri::command]
pub(super) async fn get_web_access_status(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    let server = state.web_access_server.lock().await;
    Ok(status(server.as_ref()))
}

#[tauri::command]
pub(super) async fn start_web_access(
    app: AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    let mut server = state.web_access_server.lock().await;
    if server.is_none() {
        let handle = start_web_access_server(
            WebAccessServerConfig::default(),
            state.application_commands.clone(),
            state.application_change_hub.clone(),
            Arc::new(RejectAllWebSessions),
            Arc::new(TauriSpaAssets { app }),
        )
        .await
        .map_err(|error| error.to_string())?;
        *server = Some(handle);
    }
    Ok(status(server.as_ref()))
}

#[tauri::command]
pub(super) async fn stop_web_access(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    let server = state.web_access_server.lock().await.take();
    if let Some(server) = server {
        server.shutdown().await.map_err(|error| error.to_string())?;
    }
    Ok(status(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopped_status_does_not_invent_an_address() {
        assert_eq!(
            serde_json::to_value(status(None)).expect("serialize status"),
            serde_json::json!({ "running": false, "url": null })
        );
    }
}
