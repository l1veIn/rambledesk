//! Isolated browser acceptance fixture; no model/API account and no user database.
//! Build the web bundle, then run with RAMBLEDESK_MANAGED_PREVIEW=1.
use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_local_server::*;
use rambledesk_storage::SqliteFeedbackStore;
use std::{
    collections::BTreeMap,
    path::{Component, PathBuf},
    sync::Arc,
};

struct Assets(PathBuf);
impl SpaAssetSource for Assets {
    fn load(&self, path: &str) -> Option<SpaAsset> {
        let relative = std::path::Path::new(path);
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return None;
        }
        let path = self.0.join(relative).canonicalize().ok()?;
        if !path.starts_with(&self.0) {
            return None;
        }
        let mime = match path.extension()?.to_str()? {
            "html" => "text/html; charset=utf-8",
            "js" => "text/javascript; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "webp" => "image/webp",
            "wasm" => "application/wasm",
            "json" => "application/json",
            _ => "application/octet-stream",
        };
        Some(SpaAsset {
            bytes: std::fs::read(path).ok()?,
            mime_type: mime.into(),
            content_security_policy: None,
            cache_policy: SpaAssetCachePolicy::NoStore,
        })
    }
}
fn main() -> anyhow::Result<()> {
    if rambledesk_feedback_client::process_requested() {
        std::process::exit(rambledesk_feedback_client::run_process());
    }
    run_preview()
}

#[tokio::main]
async fn run_preview() -> anyhow::Result<()> {
    anyhow::ensure!(
        std::env::var("RAMBLEDESK_MANAGED_PREVIEW").as_deref() == Ok("1"),
        "Set RAMBLEDESK_MANAGED_PREVIEW=1 to run this isolated fixture"
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let assets = Assets(root.join("apps/desktop/dist").canonicalize()?);
    let directory = tempfile::tempdir()?;
    let store = Arc::new(SqliteFeedbackStore::connect(&directory.path().join("db.sqlite")).await?);
    let hub = Arc::new(ApplicationChangeHub::new());
    let feedback = (*store)
        .clone()
        .into_application()
        .with_change_observer(hub.clone());
    let provider = Arc::new(LocalManagedFeedbackProvider::new(feedback.clone()));
    let local = start_server_with_managed(
        ServerConfig::new(AccessToken::generate()).with_port(0),
        feedback.clone(),
        provider.clone(),
    )
    .await?;
    let sessions = SessionApplication::new(
        store.clone(),
        store.clone(),
        Arc::new(AcpSessionDriver::with_feedback_companion(
            std::env::current_exe()?,
        )),
    )
    .with_change_observer(hub.clone())
    .with_feedback_provider(provider)
    .with_deliveries(store.clone())
    .with_deletions(store.clone())
    .with_recovery(store.clone());
    sessions.recover_runtime().await?;
    sessions.start_delivery_worker().await?;
    let config = sessions
        .save_agent_config(SaveAgentConfigInput {
            catalog_id: None,
            id: None,
            name: "Local ACP fixture".into(),
            host_id: "dsh".into(),
            protocol: SessionProtocol::Acp,
            enabled: true,
            command: "node".into(),
            args: vec![
                root.join("crates/rambledesk-acp/tests/fixtures/agent.mjs")
                    .to_string_lossy()
                    .into_owned(),
                "load".into(),
            ],
            env: BTreeMap::new(),
        })
        .await?;
    for title in ["Website project", "CLI project"] {
        let project = directory.path().join(title);
        std::fs::create_dir(&project)?;
        let snapshot = sessions
            .create_session(CreateManagedSessionInput {
                agent_config_id: config.id.clone(),
                cwd: project.to_string_lossy().into_owned(),
                title: title.into(),
            })
            .await?;
        anyhow::ensure!(
            snapshot.runtime.connection == SessionConnectionState::Connected,
            "Fixture launch failed: {:?}",
            snapshot.runtime.last_error
        );
        if title == "Website project" {
            feedback.request_managed_feedback(&ManagedFeedbackScope {
                session_id: snapshot.session.session_id.clone(),
                host_id: snapshot.session.host_id.clone(),
                host_session_id: snapshot.session.host_session_id.clone(),
            }, RequestFeedbackInput {
                request_id: None, host_id: None, host_session_id: String::new(),
                title: Some("Review the website fixture".into()),
                what_happened: "The isolated preview is ready. Review this request, submit feedback, and inspect its managed delivery status.".into(),
                actions: vec![ActionInput { id: "review".into(), instruction: "Review the preview and provide feedback".into() }], context_refs: vec![], attachments: vec![], source_hint: None,
                allow_finish: true, final_summary: Some("Preview review completed".into()),
            }).await?;
        }
    }
    let commands = Arc::new(
        ApplicationCommandFacade::new(
            feedback.clone(),
            WorkbenchTerminalOperations::without_observer(feedback),
            vec![],
        )
        .with_sessions(sessions.clone()),
    );
    let token = AccessToken::generate();
    let web_sessions = Arc::new(WebSessionManager::new(
        DurableWebAccessToken::parse(token.secret())?,
        hub.metadata().runtime_generation,
    ));
    let web = start_web_access_server(
        WebAccessServerConfig {
            port: 0,
            ..Default::default()
        },
        commands,
        hub,
        web_sessions,
        Arc::new(assets),
    )
    .await?;
    println!("PREVIEW {}", web.origin());
    println!("PREVIEW_TOKEN {}", token.secret());
    println!("PREVIEW_DIRECTORY {}", directory.path().display());
    // Creating this marker also permits graceful shutdown from a non-interactive shell.
    let stop_marker = directory.path().join("stop-preview");
    tokio::select! {
        signal = tokio::signal::ctrl_c() => signal?,
        _ = async { while !stop_marker.exists() { tokio::time::sleep(std::time::Duration::from_millis(250)).await; } } => {}
    }
    sessions.shutdown().await?;
    web.shutdown().await?;
    local.shutdown().await?;
    store.close().await;
    Ok(())
}
