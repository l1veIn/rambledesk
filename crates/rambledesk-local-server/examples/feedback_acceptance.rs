//! Disposable HTTP + SQLite acceptance fixture, never a product server entry point.
//! See docs/quality/FEEDBACK_ACCEPTANCE.md and scripts/feedback-acceptance.py.

use std::{
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use rambledesk_core::{
    ActionInput, AddAttachmentInput, ApplicationChangeHub, ApplicationCommandFacade,
    ApplicationHostProfileView, FeedbackApplication, RequestAttachmentInput, RequestFeedbackInput,
    SaveDraftInput, WorkbenchTerminalOperations,
};
use rambledesk_local_server::{
    AccessToken, DurableWebAccessToken, SpaAsset, SpaAssetCachePolicy, SpaAssetSource,
    WebAccessServerConfig, WebSessionManager, start_web_access_server,
};
use rambledesk_storage::SqliteFeedbackStore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct DiskAssets(PathBuf);

impl SpaAssetSource for DiskAssets {
    fn load(&self, path: &str) -> Option<SpaAsset> {
        let relative = Path::new(path);
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return None;
        }
        let file = self.0.join(relative).canonicalize().ok()?;
        if !file.starts_with(&self.0) || !file.is_file() {
            return None;
        }
        let mime_type = match file.extension()?.to_str()? {
            "html" => "text/html; charset=utf-8",
            "js" | "mjs" => "text/javascript; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "json" | "map" => "application/json",
            "wasm" => "application/wasm",
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "ico" => "image/x-icon",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            _ => "application/octet-stream",
        };
        Some(SpaAsset {
            bytes: std::fs::read(file).ok()?,
            mime_type: mime_type.into(),
            content_security_policy: None,
            // Dist is read on every request, allowing an explicitly rebuilt UI to be inspected.
            cache_policy: SpaAssetCachePolicy::NoStore,
        })
    }
}

fn paragraph(text: &str) -> Value {
    json!({"type": "paragraph", "content": [{"type": "text", "text": text}]})
}

async fn seed(application: &FeedbackApplication) -> anyhow::Result<Vec<Value>> {
    let mut requests = Vec::new();
    for (purpose, title) in [
        ("ordinary", "01 · Ordinary feedback"),
        ("long", "02 · Long structured draft (240 paragraphs)"),
        ("attachment", "03 · Draft and request attachments"),
        ("cancel", "04 · Cancellation"),
    ] {
        let request_id = Uuid::now_v7().to_string();
        application.request_feedback(RequestFeedbackInput {
            request_id: Some(request_id.clone()),
            host_id: Some("acceptance-external".into()),
            host_session_id: format!("acceptance-{purpose}"),
            title: Some(title.into()),
            what_happened: "Isolated fixture: real application, HTTP and SQLite; no real Agent, continuation, user database or native device.".into(),
            actions: vec![ActionInput { id: "review".into(), instruction: "Edit, save, refresh, then submit and inspect the published package.".into() }],
            context_refs: vec![],
            attachments: if purpose == "attachment" {
                vec![RequestAttachmentInput {
                    file_name: "agent-review.md".into(),
                    markdown: Some("# Agent review fixture\n\nCheck attachment preservation.\n".into()),
                    contents_base64: None, path: None,
                }]
            } else { vec![] },
            source_hint: Some("feedback-acceptance fixture v1".into()),
            allow_finish: false, final_summary: None,
        }).await?;
        let mut content = vec![json!({
            "type": "heading", "attrs": {"level": 2},
            "content": [{"type": "text", "text": title}]
        })];
        let count = if purpose == "long" { 240 } else { 2 };
        let mut markdown = format!("## {title}\n\n");
        for index in 1..=count {
            let text = format!(
                "Paragraph {index:03}: verify structured draft recovery, responsive editing, and stable request ownership. 中文反馈与 English text remain intact."
            );
            content.push(paragraph(&text));
            markdown.push_str(&format!("{text}\n\n"));
        }
        let mut revision = 0;
        let mut attachment_ids = Vec::new();
        if purpose == "attachment" {
            let workspace = application
                .add_feedback_attachment(AddAttachmentInput {
                    request_id: request_id.clone(),
                    file_name: "feedback-evidence.txt".into(),
                    contents: b"Deterministic feedback attachment fixture.\n".to_vec(),
                    expected_revision: revision,
                })
                .await?;
            revision = workspace.draft.saved_revision;
            let attachment = &workspace.attachments[0];
            attachment_ids.push(attachment.attachment_id.clone());
            content.push(json!({"type": "paragraph", "content": [{
                "type": "attachmentFile", "attrs": {
                    "attachmentId": attachment.attachment_id,
                    "fileName": attachment.file_name, "mediaType": attachment.media_type
                }
            }]}));
            markdown.push_str(&format!(
                "[{}](attachment://{})\n",
                attachment.file_name, attachment.attachment_id
            ));
        }
        let document =
            json!({"schemaVersion": 2, "doc": {"type": "doc", "content": content}}).to_string();
        let saved = application
            .save_feedback_draft(SaveDraftInput {
                request_id: request_id.clone(),
                document_json: document.clone(),
                body_markdown: markdown.clone(),
                expected_revision: revision,
            })
            .await?;
        requests.push(json!({
            "purpose": purpose, "requestId": request_id, "title": title,
            "savedRevision": saved.saved_revision, "paragraphs": count,
            "documentBytes": document.len(), "markdownBytes": markdown.len(),
            "documentSha256": hex::encode(Sha256::digest(document.as_bytes())),
            "markdownSha256": hex::encode(Sha256::digest(markdown.as_bytes())),
            "attachmentIds": attachment_ids,
        }));
    }
    Ok(requests)
}

fn write_secret(path: &Path, value: &str) -> anyhow::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(value.as_bytes())?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    anyhow::ensure!(
        std::env::var("RAMBLEDESK_FEEDBACK_ACCEPTANCE").as_deref() == Ok("1"),
        "Use scripts/feedback-acceptance.py start (isolated test fixture only)"
    );
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dist = std::env::var_os("RAMBLEDESK_ACCEPTANCE_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| repository.join("apps/desktop/dist"))
        .canonicalize()?;
    anyhow::ensure!(
        dist.join("index.html").is_file(),
        "Build apps/desktop/dist first"
    );
    let directory = tempfile::Builder::new()
        .prefix("rambledesk-feedback-acceptance-")
        .tempdir()?;
    let database = directory.path().join("validation.sqlite3");
    let store = SqliteFeedbackStore::connect(&database).await?;
    let changes = Arc::new(ApplicationChangeHub::new());
    let application = store
        .clone()
        .into_application()
        .with_change_observer(changes.clone());
    let requests = seed(&application).await?;
    let commands = Arc::new(ApplicationCommandFacade::new(
        application.clone(),
        WorkbenchTerminalOperations::without_observer(application),
        vec![ApplicationHostProfileView {
            id: "acceptance-external".into(),
            label: "Acceptance fixture".into(),
            icon_svg: "".into(),
            default_adapter: "generic_mcp".into(),
            continuation_mode: "manual".into(),
        }],
    ));
    let token = AccessToken::generate();
    let token_file = directory.path().join("durable-token.txt");
    write_secret(&token_file, token.secret())?;
    let sessions = Arc::new(WebSessionManager::new(
        DurableWebAccessToken::parse(token.secret())?,
        changes.metadata().runtime_generation,
    ));
    let server = start_web_access_server(
        WebAccessServerConfig {
            port: 0,
            ..Default::default()
        },
        commands,
        changes,
        sessions,
        Arc::new(DiskAssets(dist.clone())),
    )
    .await?;
    let stop_file = directory.path().join("stop-fixture");
    let manifest_file = directory.path().join("acceptance.json");
    let keep = std::env::var("RAMBLEDESK_ACCEPTANCE_KEEP").as_deref() == Ok("1");
    let manifest = json!({
        "fixture": "rambledesk-feedback-acceptance-v1", "status": "running",
        "url": server.origin(), "directory": directory.path(), "database": database,
        "tokenFile": token_file, "stopFile": stop_file, "manifestFile": manifest_file,
        "pid": std::process::id(), "keep": keep, "dist": dist, "requests": requests,
        "distIndexSha256": hex::encode(Sha256::digest(std::fs::read(dist.join("index.html"))?)),
    });
    std::fs::write(&manifest_file, serde_json::to_vec_pretty(&manifest)?)?;
    println!("{}", serde_json::to_string(&manifest)?);
    std::io::stdout().flush()?;
    tokio::select! {
        signal = tokio::signal::ctrl_c() => signal?,
        _ = async {
            while !stop_file.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        } => {}
    }
    server.shutdown().await?;
    store.close().await;
    // Retained data is useful evidence, but the live credential is no longer needed.
    std::fs::remove_file(token_file)?;
    if keep {
        // The launcher appends source/build provenance after readiness.
        let mut manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_file)?)?;
        manifest["status"] = json!("stopped");
        std::fs::write(&manifest_file, serde_json::to_vec_pretty(&manifest)?)?;
        let _ = directory.keep();
    }
    Ok(())
}
