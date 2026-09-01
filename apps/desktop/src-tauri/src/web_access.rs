use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::Arc,
    time::Instant,
};

use async_trait::async_trait;
use rambledesk_local_server::{
    DurableWebAccessToken, SpaAsset, SpaAssetCachePolicy, SpaAssetSource, WebAccessServerConfig,
    WebAccessServerError, WebAccessServerHandle, WebSessionManager, start_web_access_server,
};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::WorkbenchState;

const CREDENTIAL_SERVICE: &str = "com.rambledesk.desktop.web-access";
const CREDENTIAL_ACCOUNT: &str = "web-access-durable-token";
const BROWSER_SPEECH_ASSETS: &[&str] = &[
    "browser-speech/pcm-capture.worklet.js",
    "browser-speech/sherpa.worker.js",
    "browser-speech/runtime/sherpa-onnx-asr.js",
    "browser-speech/runtime/sherpa-onnx-wasm-web.js",
    "browser-speech/runtime/sherpa-onnx-wasm-web.wasm",
];

pub(super) trait WebAccessCredentialStore: Send + Sync {
    fn load_or_create(&self) -> Result<DurableWebAccessToken, String>;
}

pub(super) struct OsWebAccessCredentialStore;

impl WebAccessCredentialStore for OsWebAccessCredentialStore {
    fn load_or_create(&self) -> Result<DurableWebAccessToken, String> {
        load_or_create_os_credential()
    }
}

#[async_trait]
trait WebAccessListener: Send {
    fn origin(&self) -> &str;
    fn is_finished(&self) -> bool;
    fn cancel(&self);
    async fn join(self: Box<Self>) -> Result<(), WebAccessServerError>;
    async fn shutdown(self: Box<Self>) -> Result<(), WebAccessServerError>;
}

#[async_trait]
impl WebAccessListener for WebAccessServerHandle {
    fn origin(&self) -> &str {
        WebAccessServerHandle::origin(self)
    }

    fn is_finished(&self) -> bool {
        WebAccessServerHandle::is_finished(self)
    }

    fn cancel(&self) {
        WebAccessServerHandle::cancel(self);
    }

    async fn join(self: Box<Self>) -> Result<(), WebAccessServerError> {
        (*self).join().await
    }

    async fn shutdown(self: Box<Self>) -> Result<(), WebAccessServerError> {
        (*self).shutdown().await
    }
}

struct ActiveWebAccess {
    listener: Box<dyn WebAccessListener>,
    sessions: Arc<WebSessionManager>,
    durable_token: DurableWebAccessToken,
    started_at: Instant,
}

impl ActiveWebAccess {
    fn new(
        listener: impl WebAccessListener + 'static,
        sessions: Arc<WebSessionManager>,
        durable_token: DurableWebAccessToken,
    ) -> Self {
        Self {
            listener: Box::new(listener),
            sessions,
            durable_token,
            started_at: Instant::now(),
        }
    }

    fn cancel(&self) {
        self.sessions.revoke_all();
        self.listener.cancel();
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
                    path.clone(),
                    SpaAsset {
                        bytes,
                        mime_type: web_asset_mime_type(&path, mime_type),
                        content_security_policy: (path == "browser-speech/sherpa.worker.js")
                            .then(|| "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'".to_owned()),
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
    paths.extend(BROWSER_SPEECH_ASSETS.iter().map(|path| (*path).to_owned()));
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

fn web_asset_mime_type(path: &str, resolver_mime_type: String) -> String {
    if path.ends_with(".wasm") {
        "application/wasm".to_owned()
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8".to_owned()
    } else {
        resolver_mime_type
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WebAccessLifecycleState {
    Stopped,
    Running,
    Failed,
}

impl WebAccessLifecycleState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WebAccessFailureCode {
    CredentialStoreUnavailable,
    AssetsUnavailable,
    AddressInUse,
    ListenerFailed,
    ShutdownFailed,
    Unknown,
}

impl WebAccessFailureCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CredentialStoreUnavailable => "credential_store_unavailable",
            Self::AssetsUnavailable => "assets_unavailable",
            Self::AddressInUse => "address_in_use",
            Self::ListenerFailed => "listener_failed",
            Self::ShutdownFailed => "shutdown_failed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebAccessLifecycleEvent {
    activity: &'static str,
    outcome: &'static str,
    error_code: Option<WebAccessFailureCode>,
    duration_ms: u64,
}

trait WebAccessLifecycleObserver: Send + Sync {
    fn record(&self, event: WebAccessLifecycleEvent);
}

struct DiagnosticWebAccessLifecycleObserver;

impl WebAccessLifecycleObserver for DiagnosticWebAccessLifecycleObserver {
    fn record(&self, event: WebAccessLifecycleEvent) {
        crate::diagnostics::record_event(
            event.activity,
            None,
            None,
            Some(event.outcome),
            event.error_code.map(WebAccessFailureCode::as_str),
            Some(event.duration_ms),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct WebAccessFailure {
    code: WebAccessFailureCode,
    message: &'static str,
}

impl WebAccessFailure {
    fn new(code: WebAccessFailureCode) -> Self {
        let message = match code {
            WebAccessFailureCode::CredentialStoreUnavailable => {
                "Secure credential storage is unavailable. Check the system credential service, then try again."
            }
            WebAccessFailureCode::AssetsUnavailable => {
                "Web Access files are unavailable. Restart or reinstall RambleDesk, then try again."
            }
            WebAccessFailureCode::AddressInUse => {
                "The local Web Access address is already in use. Close the other process, then try again."
            }
            WebAccessFailureCode::ListenerFailed => {
                "The local Web Access listener stopped unexpectedly. Try starting Web Access again."
            }
            WebAccessFailureCode::ShutdownFailed => {
                "Web Access stopped, but its listener did not shut down cleanly. You can try starting it again."
            }
            WebAccessFailureCode::Unknown => {
                "Web Access could not complete the operation. Try again or restart RambleDesk."
            }
        };
        Self { code, message }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WebAccessStatus {
    state: WebAccessLifecycleState,
    url: Option<String>,
    failure: Option<WebAccessFailure>,
}

impl WebAccessStatus {
    fn stopped() -> Self {
        Self {
            state: WebAccessLifecycleState::Stopped,
            url: None,
            failure: None,
        }
    }

    fn running(url: String) -> Self {
        Self {
            state: WebAccessLifecycleState::Running,
            url: Some(url),
            failure: None,
        }
    }

    fn failed(code: WebAccessFailureCode) -> Self {
        Self {
            state: WebAccessLifecycleState::Failed,
            url: None,
            failure: Some(WebAccessFailure::new(code)),
        }
    }
}

enum WebAccessRuntimeState {
    Stopped,
    Running(ActiveWebAccess),
    Failed(WebAccessFailureCode),
}

pub(super) struct WebAccessLifecycle {
    state: WebAccessRuntimeState,
    observer: Arc<dyn WebAccessLifecycleObserver>,
}

impl Default for WebAccessLifecycle {
    fn default() -> Self {
        Self {
            state: WebAccessRuntimeState::Stopped,
            observer: Arc::new(DiagnosticWebAccessLifecycleObserver),
        }
    }
}

pub(super) struct WebAccessDiagnosticState {
    pub(super) state: &'static str,
    pub(super) failure_code: Option<&'static str>,
}

impl WebAccessLifecycle {
    async fn status(&mut self) -> WebAccessStatus {
        self.reconcile().await;
        self.snapshot()
    }

    pub(super) async fn diagnostic_state(&mut self) -> WebAccessDiagnosticState {
        self.reconcile().await;
        match &self.state {
            WebAccessRuntimeState::Stopped => WebAccessDiagnosticState {
                state: WebAccessLifecycleState::Stopped.as_str(),
                failure_code: None,
            },
            WebAccessRuntimeState::Running(_) => WebAccessDiagnosticState {
                state: WebAccessLifecycleState::Running.as_str(),
                failure_code: None,
            },
            WebAccessRuntimeState::Failed(code) => WebAccessDiagnosticState {
                state: WebAccessLifecycleState::Failed.as_str(),
                failure_code: Some(code.as_str()),
            },
        }
    }

    async fn start<Start, StartFuture>(&mut self, start: Start) -> WebAccessStatus
    where
        Start: FnOnce() -> StartFuture,
        StartFuture: Future<Output = Result<ActiveWebAccess, WebAccessFailureCode>>,
    {
        self.reconcile().await;
        if !matches!(self.state, WebAccessRuntimeState::Running(_)) {
            let started_at = Instant::now();
            self.state = match start().await {
                Ok(active) => {
                    self.observer.record(WebAccessLifecycleEvent {
                        activity: "web_access_start",
                        outcome: "ok",
                        error_code: None,
                        duration_ms: elapsed_ms(started_at),
                    });
                    WebAccessRuntimeState::Running(active)
                }
                Err(code) => {
                    self.observer.record(WebAccessLifecycleEvent {
                        activity: "web_access_start",
                        outcome: "error",
                        error_code: Some(code),
                        duration_ms: elapsed_ms(started_at),
                    });
                    WebAccessRuntimeState::Failed(code)
                }
            };
        }
        self.snapshot()
    }

    async fn stop(&mut self) -> WebAccessStatus {
        self.reconcile().await;
        self.stop_reconciled().await;
        self.snapshot()
    }

    async fn active_token(&mut self) -> Result<DurableWebAccessToken, String> {
        self.reconcile().await;
        match &self.state {
            WebAccessRuntimeState::Running(active) => Ok(active.durable_token.clone()),
            WebAccessRuntimeState::Stopped | WebAccessRuntimeState::Failed(_) => {
                Err("Start Web Access before copying its access token.".to_owned())
            }
        }
    }

    async fn active_url(&mut self) -> Result<String, String> {
        self.reconcile().await;
        match &self.state {
            WebAccessRuntimeState::Running(active) => Ok(active.listener.origin().to_owned()),
            WebAccessRuntimeState::Stopped | WebAccessRuntimeState::Failed(_) => {
                Err("Start Web Access before opening it.".to_owned())
            }
        }
    }

    pub(super) fn cancel_active(&self) {
        if let WebAccessRuntimeState::Running(active) = &self.state {
            active.cancel();
        }
    }

    async fn reconcile(&mut self) {
        let finished = matches!(
            &self.state,
            WebAccessRuntimeState::Running(active) if active.listener.is_finished()
        );
        if !finished {
            return;
        }
        let WebAccessRuntimeState::Running(active) =
            std::mem::replace(&mut self.state, WebAccessRuntimeState::Stopped)
        else {
            unreachable!("finished Web Access state must still be running")
        };
        active.sessions.revoke_all();
        let duration_ms = elapsed_ms(active.started_at);
        match active.listener.join().await {
            Ok(()) => tracing::warn!("Web Access listener exited unexpectedly"),
            Err(error) => tracing::warn!(%error, "Web Access listener failed"),
        }
        self.observer.record(WebAccessLifecycleEvent {
            activity: "web_access_listener_failed",
            outcome: "error",
            error_code: Some(WebAccessFailureCode::ListenerFailed),
            duration_ms,
        });
        self.state = WebAccessRuntimeState::Failed(WebAccessFailureCode::ListenerFailed);
    }

    async fn stop_reconciled(&mut self) {
        let previous = std::mem::replace(&mut self.state, WebAccessRuntimeState::Stopped);
        let WebAccessRuntimeState::Running(active) = previous else {
            return;
        };
        active.sessions.revoke_all();
        let started_at = Instant::now();
        match active.listener.shutdown().await {
            Ok(()) => self.observer.record(WebAccessLifecycleEvent {
                activity: "web_access_stop",
                outcome: "ok",
                error_code: None,
                duration_ms: elapsed_ms(started_at),
            }),
            Err(error) => {
                tracing::warn!(%error, "Web Access listener did not shut down cleanly");
                self.observer.record(WebAccessLifecycleEvent {
                    activity: "web_access_stop",
                    outcome: "error",
                    error_code: Some(WebAccessFailureCode::ShutdownFailed),
                    duration_ms: elapsed_ms(started_at),
                });
                self.state = WebAccessRuntimeState::Failed(WebAccessFailureCode::ShutdownFailed);
            }
        }
    }

    fn snapshot(&self) -> WebAccessStatus {
        match &self.state {
            WebAccessRuntimeState::Stopped => WebAccessStatus::stopped(),
            WebAccessRuntimeState::Running(active) => {
                WebAccessStatus::running(active.listener.origin().to_owned())
            }
            WebAccessRuntimeState::Failed(code) => WebAccessStatus::failed(*code),
        }
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

async fn start_runtime(
    app: AppHandle,
    state: &WorkbenchState,
) -> Result<ActiveWebAccess, WebAccessFailureCode> {
    let durable_token = state
        .web_access_credential_store
        .load_or_create()
        .map_err(|error| {
            tracing::warn!(%error, "Web Access credential store is unavailable");
            WebAccessFailureCode::CredentialStoreUnavailable
        })?;
    let assets = TauriSpaAssets::new(app).map_err(|error| {
        tracing::warn!(%error, "Web Access assets are unavailable");
        WebAccessFailureCode::AssetsUnavailable
    })?;
    let sessions = Arc::new(WebSessionManager::new(
        durable_token.clone(),
        state.application_change_hub.metadata().runtime_generation,
    ));
    let server = start_web_access_server(
        WebAccessServerConfig::default(),
        state.application_commands.clone(),
        state.application_change_hub.clone(),
        sessions.clone(),
        Arc::new(assets),
    )
    .await
    .map_err(|error| {
        let code = web_access_start_failure(&error);
        tracing::warn!(%error, ?code, "Web Access listener did not start");
        code
    })?;
    Ok(ActiveWebAccess::new(server, sessions, durable_token))
}

fn web_access_start_failure(error: &WebAccessServerError) -> WebAccessFailureCode {
    match error {
        WebAccessServerError::Bind(source) if source.kind() == std::io::ErrorKind::AddrInUse => {
            WebAccessFailureCode::AddressInUse
        }
        WebAccessServerError::Bind(_)
        | WebAccessServerError::Io(_)
        | WebAccessServerError::Join(_) => WebAccessFailureCode::ListenerFailed,
        WebAccessServerError::RouteConfig(_) => WebAccessFailureCode::Unknown,
    }
}

#[tauri::command]
pub(super) async fn get_web_access_status(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    Ok(state.web_access_lifecycle.lock().await.status().await)
}

#[tauri::command]
pub(super) async fn start_web_access(
    app: AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    Ok(state
        .web_access_lifecycle
        .lock()
        .await
        .start(|| start_runtime(app, &state))
        .await)
}

#[tauri::command]
pub(super) async fn stop_web_access(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<WebAccessStatus, String> {
    Ok(state.web_access_lifecycle.lock().await.stop().await)
}

#[tauri::command]
pub(super) async fn copy_web_access_token(
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), String> {
    let token = state
        .web_access_lifecycle
        .lock()
        .await
        .active_token()
        .await?;
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(token.secret()))
        .map_err(|_| "Could not copy the Web Access token to the system clipboard.".to_owned())
}

#[tauri::command]
pub(super) async fn open_web_access(
    app: AppHandle,
    state: tauri::State<'_, WorkbenchState>,
) -> Result<(), String> {
    let url = state.web_access_lifecycle.lock().await.active_url().await?;
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
mod tests;
