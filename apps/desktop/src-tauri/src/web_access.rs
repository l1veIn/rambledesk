use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::Arc,
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
}

impl Default for WebAccessLifecycle {
    fn default() -> Self {
        Self {
            state: WebAccessRuntimeState::Stopped,
        }
    }
}

impl WebAccessLifecycle {
    async fn status(&mut self) -> WebAccessStatus {
        self.reconcile().await;
        self.snapshot()
    }

    async fn start<Start, StartFuture>(&mut self, start: Start) -> WebAccessStatus
    where
        Start: FnOnce() -> StartFuture,
        StartFuture: Future<Output = Result<ActiveWebAccess, WebAccessFailureCode>>,
    {
        self.reconcile().await;
        if !matches!(self.state, WebAccessRuntimeState::Running(_)) {
            self.state = match start().await {
                Ok(active) => WebAccessRuntimeState::Running(active),
                Err(code) => WebAccessRuntimeState::Failed(code),
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
        match active.listener.join().await {
            Ok(()) => tracing::warn!("Web Access listener exited unexpectedly"),
            Err(error) => tracing::warn!(%error, "Web Access listener failed"),
        }
        self.state = WebAccessRuntimeState::Failed(WebAccessFailureCode::ListenerFailed);
    }

    async fn stop_reconciled(&mut self) {
        let previous = std::mem::replace(&mut self.state, WebAccessRuntimeState::Stopped);
        let WebAccessRuntimeState::Running(active) = previous else {
            return;
        };
        active.sessions.revoke_all();
        if let Err(error) = active.listener.shutdown().await {
            tracing::warn!(%error, "Web Access listener did not shut down cleanly");
            self.state = WebAccessRuntimeState::Failed(WebAccessFailureCode::ShutdownFailed);
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
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;

    #[derive(Default)]
    struct FakeListenerControl {
        finished: AtomicBool,
        cancel_count: AtomicUsize,
        join_count: AtomicUsize,
        shutdown_count: AtomicUsize,
    }

    struct FakeListener {
        origin: String,
        control: Arc<FakeListenerControl>,
        join_fails: bool,
        shutdown_fails: bool,
    }

    #[async_trait]
    impl WebAccessListener for FakeListener {
        fn origin(&self) -> &str {
            &self.origin
        }

        fn is_finished(&self) -> bool {
            self.control.finished.load(Ordering::SeqCst)
        }

        fn cancel(&self) {
            self.control.cancel_count.fetch_add(1, Ordering::SeqCst);
        }

        async fn join(self: Box<Self>) -> Result<(), WebAccessServerError> {
            self.control.join_count.fetch_add(1, Ordering::SeqCst);
            if self.join_fails {
                Err(WebAccessServerError::RouteConfig(
                    "fake listener failure".to_owned(),
                ))
            } else {
                Ok(())
            }
        }

        async fn shutdown(self: Box<Self>) -> Result<(), WebAccessServerError> {
            self.control.shutdown_count.fetch_add(1, Ordering::SeqCst);
            if self.shutdown_fails {
                Err(WebAccessServerError::RouteConfig(
                    "fake shutdown failure".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
    }

    fn durable_token(byte: char) -> DurableWebAccessToken {
        DurableWebAccessToken::parse(byte.to_string().repeat(64)).expect("test token")
    }

    fn fake_active(
        token: DurableWebAccessToken,
        control: Arc<FakeListenerControl>,
        join_fails: bool,
        shutdown_fails: bool,
    ) -> (ActiveWebAccess, Arc<WebSessionManager>) {
        let sessions = Arc::new(WebSessionManager::new(token.clone(), "runtime-1"));
        let active = ActiveWebAccess::new(
            FakeListener {
                origin: "http://127.0.0.1:37643".to_owned(),
                control,
                join_fails,
                shutdown_fails,
            },
            sessions.clone(),
            token,
        );
        (active, sessions)
    }

    #[test]
    fn canonical_status_json_preserves_state_invariants_and_stable_failure_codes() {
        assert_eq!(
            serde_json::to_value(WebAccessStatus::stopped()).expect("serialize stopped status"),
            serde_json::json!({ "state": "stopped", "url": null, "failure": null })
        );
        assert_eq!(
            serde_json::to_value(WebAccessStatus::running(
                "http://127.0.0.1:37643".to_owned()
            ))
            .expect("serialize running status"),
            serde_json::json!({
                "state": "running",
                "url": "http://127.0.0.1:37643",
                "failure": null
            })
        );
        assert_eq!(
            serde_json::to_value(WebAccessStatus::failed(WebAccessFailureCode::AddressInUse))
                .expect("serialize failed status"),
            serde_json::json!({
                "state": "failed",
                "url": null,
                "failure": {
                    "code": "address_in_use",
                    "message": "The local Web Access address is already in use. Close the other process, then try again."
                }
            })
        );
    }

    #[test]
    fn every_failure_code_is_stable_and_its_message_excludes_runtime_details() {
        for (code, expected) in [
            (
                WebAccessFailureCode::CredentialStoreUnavailable,
                "credential_store_unavailable",
            ),
            (
                WebAccessFailureCode::AssetsUnavailable,
                "assets_unavailable",
            ),
            (WebAccessFailureCode::AddressInUse, "address_in_use"),
            (WebAccessFailureCode::ListenerFailed, "listener_failed"),
            (WebAccessFailureCode::ShutdownFailed, "shutdown_failed"),
            (WebAccessFailureCode::Unknown, "unknown"),
        ] {
            let failure = WebAccessFailure::new(code);
            assert_eq!(
                serde_json::to_value(failure.code).expect("serialize failure code"),
                serde_json::json!(expected)
            );
            assert!(!failure.message.contains("/Users/private"));
            assert!(!failure.message.contains(&"a".repeat(64)));
        }
    }

    #[tokio::test]
    async fn start_is_idempotent_and_retry_replaces_a_failed_state() {
        let starts = Arc::new(AtomicUsize::new(0));
        let mut lifecycle = WebAccessLifecycle::default();

        let failed = lifecycle
            .start(|| async { Err(WebAccessFailureCode::AddressInUse) })
            .await;
        assert_eq!(
            failed,
            WebAccessStatus::failed(WebAccessFailureCode::AddressInUse)
        );

        let (active, _) = fake_active(
            durable_token('a'),
            Arc::new(FakeListenerControl::default()),
            false,
            false,
        );
        let starts_for_retry = starts.clone();
        let running = lifecycle
            .start(|| async move {
                starts_for_retry.fetch_add(1, Ordering::SeqCst);
                Ok(active)
            })
            .await;
        assert_eq!(
            running,
            WebAccessStatus::running("http://127.0.0.1:37643".to_owned())
        );

        let still_running = lifecycle
            .start(|| async {
                panic!("an already-running lifecycle must not start another listener")
            })
            .await;
        assert_eq!(still_running, running);
        assert_eq!(starts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn status_reconciles_a_finished_listener_and_revokes_its_sessions() {
        let control = Arc::new(FakeListenerControl::default());
        let token = durable_token('a');
        let (active, sessions) = fake_active(token.clone(), control.clone(), false, false);
        let session_token = sessions
            .issue_session(token.secret())
            .expect("issue session before listener failure");
        let authorization = sessions
            .authorize(&session_token)
            .expect("authorize session before listener failure");
        let mut lifecycle = WebAccessLifecycle::default();
        lifecycle.start(|| async { Ok(active) }).await;

        control.finished.store(true, Ordering::SeqCst);
        let reconciled = lifecycle.status().await;

        assert_eq!(
            reconciled,
            WebAccessStatus::failed(WebAccessFailureCode::ListenerFailed)
        );
        assert_eq!(control.join_count.load(Ordering::SeqCst), 1);
        assert!(!authorization.is_active());
        assert!(lifecycle.active_token().await.is_err());
    }

    #[tokio::test]
    async fn the_active_token_remains_the_token_used_by_the_running_session_manager() {
        let token_a = durable_token('a');
        let token_b = durable_token('b');
        let (active, sessions) = fake_active(
            token_a.clone(),
            Arc::new(FakeListenerControl::default()),
            false,
            false,
        );
        let mut lifecycle = WebAccessLifecycle::default();
        lifecycle.start(|| async { Ok(active) }).await;

        assert_eq!(
            lifecycle
                .active_token()
                .await
                .expect("active token")
                .secret(),
            token_a.secret()
        );
        assert!(sessions.issue_session(token_a.secret()).is_some());
        assert!(sessions.issue_session(token_b.secret()).is_none());
    }

    #[tokio::test]
    async fn a_successful_stop_revokes_sessions_and_cannot_report_running() {
        let control = Arc::new(FakeListenerControl::default());
        let token = durable_token('a');
        let (active, sessions) = fake_active(token.clone(), control.clone(), false, false);
        let session_token = sessions
            .issue_session(token.secret())
            .expect("issue session before stop");
        let authorization = sessions
            .authorize(&session_token)
            .expect("authorize session before stop");
        let mut lifecycle = WebAccessLifecycle::default();
        lifecycle.start(|| async { Ok(active) }).await;

        let stopped = lifecycle.stop().await;

        assert_eq!(stopped, WebAccessStatus::stopped());
        assert_eq!(control.shutdown_count.load(Ordering::SeqCst), 1);
        assert!(!authorization.is_active());
        assert_eq!(lifecycle.status().await, WebAccessStatus::stopped());
    }

    #[tokio::test]
    async fn process_exit_cancellation_revokes_sessions_and_cancels_the_listener() {
        let control = Arc::new(FakeListenerControl::default());
        let token = durable_token('a');
        let (active, sessions) = fake_active(token.clone(), control.clone(), false, false);
        let session_token = sessions
            .issue_session(token.secret())
            .expect("issue session before process exit");
        let authorization = sessions
            .authorize(&session_token)
            .expect("authorize session before process exit");
        let mut lifecycle = WebAccessLifecycle::default();
        lifecycle.start(|| async { Ok(active) }).await;

        lifecycle.cancel_active();

        assert_eq!(control.cancel_count.load(Ordering::SeqCst), 1);
        assert!(!authorization.is_active());
    }

    #[tokio::test]
    async fn a_shutdown_error_reports_the_real_failed_state_and_allows_retry() {
        let control = Arc::new(FakeListenerControl::default());
        let (active, _) = fake_active(durable_token('a'), control.clone(), false, true);
        let mut lifecycle = WebAccessLifecycle::default();
        lifecycle.start(|| async { Ok(active) }).await;

        let failed = lifecycle.stop().await;

        assert_eq!(
            failed,
            WebAccessStatus::failed(WebAccessFailureCode::ShutdownFailed)
        );
        assert_eq!(control.shutdown_count.load(Ordering::SeqCst), 1);
        assert_eq!(lifecycle.status().await, failed);
        assert!(lifecycle.active_url().await.is_err());

        let (retry, _) = fake_active(
            durable_token('b'),
            Arc::new(FakeListenerControl::default()),
            false,
            false,
        );
        let running = lifecycle.start(|| async { Ok(retry) }).await;
        assert_eq!(running.state, WebAccessLifecycleState::Running);
    }

    #[test]
    fn server_start_errors_map_to_stable_failure_codes() {
        let address_in_use = WebAccessServerError::Bind(std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            "secret path must not escape",
        ));
        assert_eq!(
            web_access_start_failure(&address_in_use),
            WebAccessFailureCode::AddressInUse
        );
        assert_eq!(
            web_access_start_failure(&WebAccessServerError::RouteConfig(
                "internal detail".to_owned()
            )),
            WebAccessFailureCode::Unknown
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
        let mut readable = HashMap::from([
            ("index.html", b"<main>Workbench</main>".to_vec()),
            ("assets/app-abc12345.js", b"export {}".to_vec()),
            ("assets/app-def67890.css", b"body{}".to_vec()),
        ]);
        for path in BROWSER_SPEECH_ASSETS {
            readable.insert(path, b"browser speech asset".to_vec());
        }
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
        let requested = requested.into_inner();
        assert!(requested.contains(&"assets/app-abc12345.js".to_owned()));
        assert!(requested.contains(&"browser-speech/sherpa.worker.js".to_owned()));
        let wasm = assets
            .load("browser-speech/runtime/sherpa-onnx-wasm-web.wasm")
            .expect("wasm asset");
        assert_eq!(wasm.mime_type, "application/wasm");
        let worker = assets
            .load("browser-speech/sherpa.worker.js")
            .expect("worker asset");
        assert!(
            worker
                .content_security_policy
                .as_deref()
                .is_some_and(|policy| {
                    policy.contains("script-src 'self' 'wasm-unsafe-eval'")
                        && policy.contains("connect-src 'self'")
                })
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
