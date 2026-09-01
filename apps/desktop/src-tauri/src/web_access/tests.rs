use std::sync::Mutex;
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

struct NoopLifecycleObserver;

impl WebAccessLifecycleObserver for NoopLifecycleObserver {
    fn record(&self, _event: WebAccessLifecycleEvent) {}
}

#[derive(Default)]
struct RecordingLifecycleObserver {
    events: Mutex<Vec<WebAccessLifecycleEvent>>,
}

impl WebAccessLifecycleObserver for RecordingLifecycleObserver {
    fn record(&self, event: WebAccessLifecycleEvent) {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(event);
    }
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

fn test_lifecycle() -> WebAccessLifecycle {
    WebAccessLifecycle {
        state: WebAccessRuntimeState::Stopped,
        observer: Arc::new(NoopLifecycleObserver),
    }
}

fn lifecycle_with_observer(observer: Arc<dyn WebAccessLifecycleObserver>) -> WebAccessLifecycle {
    WebAccessLifecycle {
        state: WebAccessRuntimeState::Stopped,
        observer,
    }
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
    let mut lifecycle = test_lifecycle();

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
        .start(|| async { panic!("an already-running lifecycle must not start another listener") })
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
    let mut lifecycle = test_lifecycle();
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
    let mut lifecycle = test_lifecycle();
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
    let mut lifecycle = test_lifecycle();
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
    let mut lifecycle = test_lifecycle();
    lifecycle.start(|| async { Ok(active) }).await;

    lifecycle.cancel_active();

    assert_eq!(control.cancel_count.load(Ordering::SeqCst), 1);
    assert!(!authorization.is_active());
}

#[tokio::test]
async fn a_shutdown_error_reports_the_real_failed_state_and_allows_retry() {
    let control = Arc::new(FakeListenerControl::default());
    let (active, _) = fake_active(durable_token('a'), control.clone(), false, true);
    let mut lifecycle = test_lifecycle();
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

#[tokio::test]
async fn lifecycle_events_contain_only_stable_metadata_for_start_stop_and_failure() {
    let observer = Arc::new(RecordingLifecycleObserver::default());
    let mut lifecycle = lifecycle_with_observer(observer.clone());

    lifecycle
        .start(|| async { Err(WebAccessFailureCode::AddressInUse) })
        .await;
    let (shutdown_failure, _) = fake_active(
        durable_token('a'),
        Arc::new(FakeListenerControl::default()),
        false,
        true,
    );
    lifecycle.start(|| async { Ok(shutdown_failure) }).await;
    lifecycle.stop().await;

    let failed_control = Arc::new(FakeListenerControl::default());
    let (unexpected_failure, _) =
        fake_active(durable_token('b'), failed_control.clone(), false, false);
    lifecycle.start(|| async { Ok(unexpected_failure) }).await;
    failed_control.finished.store(true, Ordering::SeqCst);
    lifecycle.status().await;

    let events = observer
        .events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.activity, event.outcome, event.error_code))
            .collect::<Vec<_>>(),
        vec![
            (
                "web_access_start",
                "error",
                Some(WebAccessFailureCode::AddressInUse),
            ),
            ("web_access_start", "ok", None),
            (
                "web_access_stop",
                "error",
                Some(WebAccessFailureCode::ShutdownFailed),
            ),
            ("web_access_start", "ok", None),
            (
                "web_access_listener_failed",
                "error",
                Some(WebAccessFailureCode::ListenerFailed),
            ),
        ]
    );
    assert_eq!(events.len(), 5);
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
