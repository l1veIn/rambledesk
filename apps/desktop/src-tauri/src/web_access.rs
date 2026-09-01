use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use rambledesk_local_server::{
    DurableWebAccessToken, SpaAsset, SpaAssetCachePolicy, SpaAssetSource, WebAccessServerConfig,
    WebAccessServerHandle, WebSessionManager, start_web_access_server,
};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::WorkbenchState;

const CREDENTIAL_SERVICE: &str = "com.rambledesk.desktop.web-access";
const CREDENTIAL_ACCOUNT: &str = "web-access-durable-token";

pub(super) trait WebAccessCredentialStore: Send + Sync {
    fn load_or_create(&self) -> Result<DurableWebAccessToken, String>;
}

pub(super) struct OsWebAccessCredentialStore;

impl WebAccessCredentialStore for OsWebAccessCredentialStore {
    fn load_or_create(&self) -> Result<DurableWebAccessToken, String> {
        load_or_create_os_credential()
    }
}

pub(super) struct WebAccessRuntime {
    server: WebAccessServerHandle,
    sessions: Arc<WebSessionManager>,
}

impl WebAccessRuntime {
    pub(super) fn cancel(&self) {
        self.sessions.revoke_all();
        self.server.cancel();
    }
}

struct TauriSpaAssets {
    assets: HashMap<String, SpaAsset>,
}

impl TauriSpaAssets {
    fn new(app: AppHandle) -> Result<Self, String> {
        let resolver = app.asset_resolver();
        let mut entries = resolver
            .iter()
            .map(|(path, _bytes)| {
                let path = validated_asset_key(path.strip_prefix('/').unwrap_or(&path))?;
                let asset = resolver
                    .get(path.clone())
                    .ok_or_else(|| "Web Access asset inventory is inconsistent.".to_owned())?;
                Ok((path, asset.bytes, asset.mime_type))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if !entries
            .iter()
            .any(|(path, _, _)| path == ".vite/manifest.json")
        {
            let manifest = resolver
                .get(".vite/manifest.json".to_owned())
                .ok_or_else(|| {
                    "Web Access assets are unavailable. Rebuild the shared Workbench and try again."
                        .to_owned()
                })?;
            entries = dev_asset_entries(&manifest.bytes, |path| {
                resolver
                    .get(path.to_owned())
                    .map(|asset| (asset.bytes, asset.mime_type))
            })?;
        }
        Self::from_entries(entries)
    }

    fn from_entries(entries: Vec<(String, Vec<u8>, String)>) -> Result<Self, String> {
        let manifest = entries
            .iter()
            .find(|(path, _, _)| path == ".vite/manifest.json")
            .ok_or_else(|| {
                "Web Access assets are unavailable. Rebuild the shared Workbench and try again."
                    .to_owned()
            })?;
        let immutable = vite_manifest_outputs(&manifest.1)?;
        let assets = entries
            .into_iter()
            .filter(|(path, _, _)| path != ".vite/manifest.json")
            .map(|(path, bytes, mime_type)| {
                let cache_policy = if path == "index.html" {
                    SpaAssetCachePolicy::NoStore
                } else if immutable.contains(&path) {
                    SpaAssetCachePolicy::Immutable
                } else {
                    SpaAssetCachePolicy::NoCache
                };
                (
                    path,
                    SpaAsset {
                        bytes,
                        mime_type,
                        content_security_policy: None,
                        cache_policy,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        if !assets.contains_key("index.html") {
            return Err("Web Access assets do not contain the Workbench entry page.".to_owned());
        }
        Ok(Self { assets })
    }
}

fn dev_asset_entries(
    manifest_bytes: &[u8],
    mut load: impl FnMut(&str) -> Option<(Vec<u8>, String)>,
) -> Result<Vec<(String, Vec<u8>, String)>, String> {
    let outputs = vite_manifest_outputs(manifest_bytes)?;
    let mut paths = outputs.into_iter().collect::<Vec<_>>();
    paths.push("index.html".to_owned());
    paths.sort();
    paths.dedup();
    let mut entries = vec![(
        ".vite/manifest.json".to_owned(),
        manifest_bytes.to_vec(),
        "application/json".to_owned(),
    )];
    for path in paths {
        let (bytes, mime_type) = load(&path).ok_or_else(|| {
            format!("Web Access asset manifest references an unreadable asset: {path}")
        })?;
        entries.push((path, bytes, mime_type));
    }
    Ok(entries)
}
impl SpaAssetSource for TauriSpaAssets {
    fn load(&self, path: &str) -> Option<SpaAsset> {
        self.assets.get(path).cloned()
    }
}

fn validated_asset_key(path: &str) -> Result<String, String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(['\\', '\0', '?', '#'])
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err("Web Access asset manifest contains an unsafe path.".to_owned());
    }
    Ok(path.to_owned())
}

fn vite_manifest_outputs(bytes: &[u8]) -> Result<HashSet<String>, String> {
    let manifest: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "Web Access asset manifest is invalid.".to_owned())?;
    let records = manifest
        .as_object()
        .ok_or_else(|| "Web Access asset manifest is invalid.".to_owned())?;
    let mut outputs = HashSet::new();
    for record in records.values() {
        let Some(record) = record.as_object() else {
            return Err("Web Access asset manifest is invalid.".to_owned());
        };
        if let Some(file) = record.get("file").and_then(serde_json::Value::as_str) {
            outputs.insert(validated_asset_key(file)?);
        }
        for field in ["css", "assets"] {
            if let Some(values) = record.get(field).and_then(serde_json::Value::as_array) {
                for value in values.iter().filter_map(serde_json::Value::as_str) {
                    outputs.insert(validated_asset_key(value)?);
                }
            }
        }
    }
    Ok(outputs)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WebAccessStatus {
    running: bool,
    url: Option<String>,
}

fn status(runtime: Option<&WebAccessRuntime>) -> WebAccessStatus {
    WebAccessStatus {
        running: runtime.is_some(),
        url: runtime.map(|runtime| runtime.server.origin().to_owned()),
    }
}

#[tauri::command]
pub(super) async fn get_web_access_status(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    let runtime = state.web_access_runtime.lock().await;
    Ok(status(runtime.as_ref()))
}

#[tauri::command]
pub(super) async fn start_web_access(
    app: AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    let mut runtime = state.web_access_runtime.lock().await;
    if runtime.is_none() {
        let durable_token = state.web_access_credential_store.load_or_create()?;
        let sessions = Arc::new(WebSessionManager::new(
            durable_token,
            state.application_change_hub.metadata().runtime_generation,
        ));
        let server = start_web_access_server(
            WebAccessServerConfig::default(),
            state.application_commands.clone(),
            state.application_change_hub.clone(),
            sessions.clone(),
            Arc::new(TauriSpaAssets::new(app)?),
        )
        .await
        .map_err(|error| error.to_string())?;
        *runtime = Some(WebAccessRuntime { server, sessions });
    }
    Ok(status(runtime.as_ref()))
}

#[tauri::command]
pub(super) async fn stop_web_access(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    let runtime = state.web_access_runtime.lock().await.take();
    if let Some(runtime) = runtime {
        runtime.sessions.revoke_all();
        runtime
            .server
            .shutdown()
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(status(None))
}

#[tauri::command]
pub(super) async fn copy_web_access_token(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), String> {
    if state.web_access_runtime.lock().await.is_none() {
        return Err("Start Web Access before copying its access token.".to_owned());
    }
    let token = state.web_access_credential_store.load_or_create()?;
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(token.secret()))
        .map_err(|_| "Could not copy the Web Access token to the system clipboard.".to_owned())
}

#[tauri::command]
pub(super) async fn open_web_access(
    app: AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), String> {
    let runtime = state.web_access_runtime.lock().await;
    let url = runtime
        .as_ref()
        .map(|runtime| runtime.server.origin())
        .ok_or_else(|| "Start Web Access before opening it.".to_owned())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "Could not open Web Access in the default browser.".to_owned())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn load_or_create_os_credential() -> Result<DurableWebAccessToken, String> {
    let entry = keyring::Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_ACCOUNT)
        .map_err(|_| secure_storage_error())?;
    match entry.get_password() {
        Ok(token) => DurableWebAccessToken::parse(token).map_err(|_| secure_storage_error()),
        Err(keyring::Error::NoEntry) => {
            let token = DurableWebAccessToken::generate();
            entry
                .set_password(token.secret())
                .map_err(|_| secure_storage_error())?;
            Ok(token)
        }
        Err(_) => Err(secure_storage_error()),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn load_or_create_os_credential() -> Result<DurableWebAccessToken, String> {
    Err(secure_storage_error())
}

fn secure_storage_error() -> String {
    "Secure credential storage is unavailable; Web Access was not started.".to_owned()
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

    #[test]
    fn secure_storage_errors_never_include_a_credential() {
        let token = DurableWebAccessToken::generate();
        assert!(!secure_storage_error().contains(token.secret()));
        assert!(!format!("{token:?}").contains(token.secret()));
    }

    #[test]
    fn exact_manifest_membership_prevents_tauri_fallback_and_false_immutable_assets() {
        let outputs = vite_manifest_outputs(
            br#"{
                "src/main.ts": {"file":"assets/app-abc12345.js","css":["assets/app-def67890.css"]}
            }"#,
        )
        .expect("manifest");
        assert!(outputs.contains("assets/app-abc12345.js"));
        assert!(outputs.contains("assets/app-def67890.css"));
        assert!(!outputs.contains("assets/release-notes-important-name.js"));

        let exact = HashMap::from([("index.html".to_owned(), SpaAssetCachePolicy::NoStore)]);
        assert!(!exact.contains_key("assets/missing.js"));
    }

    #[test]
    fn dev_manifest_builds_an_exact_cached_inventory_without_request_path_fallback() {
        let readable = HashMap::from([
            ("index.html", b"<main>Workbench</main>".to_vec()),
            ("assets/app-abc12345.js", b"export {}".to_vec()),
            ("assets/app-def67890.css", b"body{}".to_vec()),
        ]);
        let requested = std::cell::RefCell::new(Vec::new());
        let entries = dev_asset_entries(
            br#"{
                "src/main.ts": {"file":"assets/app-abc12345.js","css":["assets/app-def67890.css"]}
            }"#,
            |path| {
                requested.borrow_mut().push(path.to_owned());
                readable
                    .get(path)
                    .cloned()
                    .map(|bytes| (bytes, "test/type".to_owned()))
            },
        )
        .expect("dev inventory");
        let assets = TauriSpaAssets::from_entries(entries).expect("assets");

        assert!(assets.load("index.html").is_some());
        assert_eq!(
            assets
                .load("assets/app-abc12345.js")
                .expect("script")
                .cache_policy,
            SpaAssetCachePolicy::Immutable,
        );
        assert!(assets.load("assets/missing.js").is_none());
        assert_eq!(
            requested.into_inner(),
            [
                "assets/app-abc12345.js",
                "assets/app-def67890.css",
                "index.html",
            ],
        );
    }

    #[test]
    fn manifest_paths_reject_non_relative_and_ambiguous_forms() {
        for path in [
            "/absolute.js",
            "../escape.js",
            "assets/./app.js",
            "assets\\app.js",
            "assets/app.js?query",
            "assets/app.js#fragment",
            "assets/\0app.js",
        ] {
            let manifest = serde_json::json!({ "entry": { "file": path } });
            assert!(
                vite_manifest_outputs(manifest.to_string().as_bytes()).is_err(),
                "{path:?}"
            );
        }
    }
}
