use std::{collections::BTreeMap, process::Stdio, sync::Arc, time::Duration};

use anyhow::{Context, bail};
use rambledesk_core::{
    AgentConfig, FeedbackRepository, ManagedFeedbackEndpoint, ManagedFeedbackProvider,
    NewManagedSession, SessionProtocol, SessionRecord, SessionRepository,
};
use rambledesk_local_server::{
    AccessToken, LocalManagedFeedbackProvider, ServerConfig, ServerHandle,
    start_server_with_managed,
};
use rambledesk_storage::SqliteFeedbackStore;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

#[path = "support/native_pi.rs"]
mod native_pi;
#[cfg(unix)]
#[path = "support/owned_group.rs"]
mod owned_group;

struct Fixture {
    _directory: tempfile::TempDir,
    store: SqliteFeedbackStore,
    server: ServerHandle,
    provider: Arc<LocalManagedFeedbackProvider>,
    sessions: Vec<SessionRecord>,
}
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let store = SqliteFeedbackStore::connect(&directory.path().join("fixture.sqlite3")).await?;
        store
            .save_agent_config(AgentConfig {
                catalog_id: None,
                id: "config".into(),
                name: "Fixture".into(),
                host_id: "dsh".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: "fixture".into(),
                args: vec![],
                env: BTreeMap::new(),
                created_at: "2026-09-04T00:00:00Z".into(),
                updated_at: "2026-09-04T00:00:00Z".into(),
            })
            .await?;
        let mut sessions = vec![];
        for id in ["managed-a", "managed-b"] {
            sessions.push(
                store
                    .create_managed_session(NewManagedSession {
                        session_id: id.into(),
                        agent_config_id: "config".into(),
                        cwd: directory.path().to_string_lossy().into_owned(),
                        title: id.into(),
                        created_at: "2026-09-04T00:00:00Z".into(),
                    })
                    .await?,
            );
        }
        let app = store.clone().into_application();
        let provider = Arc::new(LocalManagedFeedbackProvider::new(app.clone()));
        let server = start_server_with_managed(
            ServerConfig::new(AccessToken::generate()).with_port(0),
            app,
            provider.clone(),
        )
        .await?;
        Ok(Self {
            _directory: directory,
            store,
            server,
            provider,
            sessions,
        })
    }
}

struct Companion {
    // Drop before Child: an unreaped leader pins the test-owned group ID.
    #[cfg(unix)]
    group: Option<owned_group::OwnedGroup>,
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    stderr: tokio::task::JoinHandle<Vec<u8>>,
    next_id: u32,
}
impl Companion {
    fn spawn_pi(
        endpoint: &ManagedFeedbackEndpoint,
        extension: &std::path::Path,
        heartbeat: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let node = rambledesk_core::find_executable("node").context("Node fixture")?;
        let native =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pi_rpc.mjs");
        let mut command = Command::new(env!("CARGO_BIN_EXE_rambledesk"));
        command
            .args([
                "--mode",
                "rpc",
                "--no-themes",
                "--session",
                "fixture-session.json",
            ])
            .env(rambledesk_acp::pi_wrapper::WRAPPER_ENV, "1")
            .env(rambledesk_acp::pi_wrapper::COMMAND_ENV, node)
            .env(
                rambledesk_acp::pi_wrapper::ARGS_ENV,
                serde_json::to_string(&vec![native])?,
            )
            .env(rambledesk_acp::pi_wrapper::EXTENSION_ENV, extension)
            .env("PI_ACP_PI_COMMAND", env!("CARGO_BIN_EXE_rambledesk"))
            .env(rambledesk_mcp::managed_stdio::URL_ENV, &endpoint.url)
            .env(
                rambledesk_mcp::managed_stdio::TOKEN_ENV,
                &endpoint.bearer_token,
            )
            .env("FIXTURE_HEARTBEAT", heartbeat)
            .env("RUST_LOG", "trace")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Production uses the ACP owner's group. This direct wrapper fixture
        // supplies that same outer ownership itself on Unix.
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn()?;
        #[cfg(unix)]
        let group = Some(owned_group::OwnedGroup::new(
            child.id().context("child ID")?,
        ));
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().context("stdout")?);
        let mut stderr = child.stderr.take().context("stderr")?;
        let stderr = tokio::spawn(async move {
            let mut bytes = vec![];
            stderr.read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        Ok(Self {
            #[cfg(unix)]
            group,
            child,
            input,
            output,
            stderr,
            next_id: 0,
        })
    }
    fn spawn(endpoint: &ManagedFeedbackEndpoint) -> anyhow::Result<Self> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rambledesk"))
            .arg("managed-mcp-stdio")
            .env(rambledesk_mcp::managed_stdio::URL_ENV, &endpoint.url)
            .env(
                rambledesk_mcp::managed_stdio::TOKEN_ENV,
                &endpoint.bearer_token,
            )
            .env(
                "RAMBLEDESK_LOCAL_SERVER_TOKEN",
                "global-token-must-not-be-used",
            )
            .env("RUST_LOG", "trace")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().context("stdout")?);
        let mut stderr = child.stderr.take().context("stderr")?;
        let stderr = tokio::spawn(async move {
            let mut bytes = vec![];
            stderr.read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        Ok(Self {
            #[cfg(unix)]
            group: None,
            child,
            input,
            output,
            stderr,
            next_id: 0,
        })
    }
    async fn write(&mut self, value: Value) -> anyhow::Result<()> {
        let mut bytes = serde_json::to_vec(&value)?;
        bytes.push(b'\n');
        self.input
            .as_mut()
            .context("stdin")?
            .write_all(&bytes)
            .await?;
        Ok(())
    }
    async fn rpc(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.next_id += 1;
        self.write(json!({"jsonrpc":"2.0","id":self.next_id,"method":method,"params":params}))
            .await?;
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let mut line = String::new();
                if self.output.read_line(&mut line).await? == 0 {
                    bail!("companion closed");
                }
                let value: Value =
                    serde_json::from_str(&line).context("stdout contains only MCP JSON")?;
                if value["id"] == self.next_id {
                    return Ok(value);
                }
            }
        })
        .await?
    }
    async fn initialize(&mut self) -> anyhow::Result<Value> {
        let value=self.rpc("initialize",json!({"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"stdio-fixture","version":"1"}})).await?;
        assert!(
            value["result"]["instructions"]
                .as_str()
                .context("instructions")?
                .contains("end the current Agent turn")
        );
        self.write(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        Ok(value)
    }
    async fn call(&mut self, name: &str, args: Value) -> anyhow::Result<Value> {
        self.rpc("tools/call", json!({"name":name,"arguments":args}))
            .await
    }
    async fn stop(mut self, secret: &str) -> anyhow::Result<()> {
        drop(self.input.take());
        #[cfg(unix)]
        drop(self.group.take());
        tokio::time::timeout(Duration::from_secs(8), self.child.wait()).await??;
        let stderr = self.stderr.await?;
        let text = String::from_utf8_lossy(&stderr);
        assert!(!text.contains(secret), "capability must never be logged");
        assert!(!text.contains("global-token-must-not-be-used"));
        assert!(
            stderr.len() < 1024,
            "diagnostics stay bounded even with RUST_LOG=trace"
        );
        Ok(())
    }
}

#[tokio::test]
async fn real_stdio_companions_preserve_private_identity_idempotency_and_revocation()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let a = fixture.provider.bind(&fixture.sessions[0]).await?;
    let b = fixture.provider.bind(&fixture.sessions[1]).await?;
    let mut first = Companion::spawn(&a)?;
    let mut second = Companion::spawn(&b)?;
    first.initialize().await?;
    second.initialize().await?;
    let listed = first.rpc("tools/list", json!({})).await?;
    let mut names: Vec<_> = listed["result"]["tools"]
        .as_array()
        .context("tools")?
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["get_feedback", "recover_feedback", "request_feedback"]
    );
    let schema = &listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "request_feedback")
        .unwrap()["inputSchema"]["properties"];
    for field in ["host_id", "host_session_id", "managed_session_id"] {
        assert!(schema.get(field).is_none());
    }
    let request_id = "b66a2bc2-474e-43e8-b8ea-ad638bda53bc";
    let payload = json!({"request_id":request_id,"what_happened":"Review the stdio fixture","actions":[{"id":"review","instruction":"Review the fixture"}],"host_id":"attacker","host_session_id":"managed-b","managed_session_id":"managed-b"});
    let result = first.call("request_feedback", payload.clone()).await?;
    assert_ne!(result["result"]["isError"], true, "{result}");
    assert!(
        result["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("End this Agent turn now")
    );
    let repeated = first.call("request_feedback", payload).await?;
    assert_eq!(
        repeated["result"]["structuredContent"]["request_id"],
        request_id
    );
    let stored = fixture.store.get_request(request_id).await?;
    assert_eq!(stored.managed_session_id.as_deref(), Some("managed-a"));
    for name in ["get_feedback", "recover_feedback"] {
        let denied = second.call(name, json!({"request_id":request_id})).await?;
        assert_eq!(
            denied["result"]["structuredContent"]["code"],
            "REQUEST_NOT_FOUND"
        );
    }
    for name in ["wait_feedback", "cancel_feedback"] {
        assert!(
            first
                .call(name, json!({"request_id":request_id}))
                .await?
                .get("error")
                .is_some()
        );
    }
    // A fresh stdio/HTTP transport still reads the same durable request.
    first.stop(&a.bearer_token).await?;
    let mut resumed = Companion::spawn(&a)?;
    resumed.initialize().await?;
    assert_eq!(
        resumed
            .call("recover_feedback", json!({"request_id":request_id}))
            .await?["result"]["structuredContent"]["request_id"],
        request_id
    );
    fixture.provider.revoke("managed-a").await?;
    let denied = resumed
        .call("get_feedback", json!({"request_id":request_id}))
        .await;
    assert!(denied.is_err() || denied.as_ref().unwrap().get("error").is_some());
    // Keep stdin open: revocation must not leave Tokio's blocking stdin reader
    // preventing the companion process from exiting.
    tokio::time::timeout(Duration::from_secs(6), resumed.child.wait()).await??;
    // Revocation cannot terminate another session's companion or grant it access.
    assert_eq!(
        second.rpc("tools/list", json!({})).await?["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    resumed.stop(&a.bearer_token).await?;
    second.stop(&b.bearer_token).await?;
    let mut stale = Companion::spawn(&a)?;
    assert!(stale.initialize().await.is_err());
    stale.stop(&a.bearer_token).await?;
    fixture.server.shutdown().await?;
    fixture.store.close().await;
    Ok(())
}

#[tokio::test]
async fn managed_pi_extension_uses_private_tools_without_wait_or_duplicate_continuation()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let a = fixture.provider.bind(&fixture.sessions[0]).await?;
    let b = fixture.provider.bind(&fixture.sessions[1]).await?;
    let extension = rambledesk_acp::pi_wrapper::install_managed_extension(
        &fixture._directory.path().join("pi-runtime"),
    )
    .await?;
    let heartbeat = fixture._directory.path().join("pi-heartbeat");
    let mut pi = Companion::spawn_pi(&a, &extension, &heartbeat)?;
    let mut other = Companion::spawn(&b)?;
    pi.initialize().await?;
    tokio::time::timeout(Duration::from_secs(3), async {
        while !heartbeat.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("owned fixture descendant starts")?;
    other.initialize().await?;
    let listed = pi.rpc("tools/list", json!({})).await?;
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 3);
    let created=pi.call("request_feedback",json!({"what_happened":"Managed Pi fixture","actions":[{"id":"review","instruction":"Review Pi fixture"}],"host_session_id":"managed-b","wait":true})).await?;
    assert_ne!(created["result"]["isError"], true, "{created}");
    assert_eq!(created["result"]["structuredContent"]["status"], "waiting");
    let id = created["result"]["structuredContent"]["request_id"]
        .as_str()
        .context("request id")?;
    assert_eq!(
        fixture
            .store
            .get_request(id)
            .await?
            .managed_session_id
            .as_deref(),
        Some("managed-a")
    );
    assert_eq!(
        other.call("get_feedback", json!({"request_id":id})).await?["result"]["structuredContent"]
            ["code"],
        "REQUEST_NOT_FOUND"
    );
    let recovered = pi.call("recover_feedback", json!({})).await?;
    assert_eq!(recovered["result"]["structuredContent"]["request_id"], id);
    fixture.provider.revoke("managed-a").await?;
    assert_eq!(
        pi.call("get_feedback", json!({"request_id":id})).await?["result"]["isError"],
        true
    );
    pi.stop(&a.bearer_token).await?;
    let before = tokio::fs::read(&heartbeat).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        before,
        tokio::fs::read(&heartbeat).await?,
        "wrapper EOF cleans the descendant tree"
    );
    assert_eq!(
        other.rpc("tools/list", json!({})).await?["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    other.stop(&b.bearer_token).await?;
    fixture.server.shutdown().await?;
    fixture.store.close().await;
    Ok(())
}
