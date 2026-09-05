//! Execute the Desktop binary itself: library/CLI tests cannot cover its early
//! dispatch, native stdout handles, or accidental Tauri/database initialization.
use std::{collections::BTreeMap, path::Path, process::Stdio, sync::Arc, time::Duration};

use anyhow::{Context, ensure};
use rambledesk_core::{
    AgentConfig, ManagedFeedbackEndpoint, ManagedFeedbackProvider, NewManagedSession,
    SessionProtocol, SessionRepository,
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

const DEADLINE: Duration = Duration::from_secs(12);

struct Fixture {
    directory: tempfile::TempDir,
    store: SqliteFeedbackStore,
    server: ServerHandle,
    endpoint: ManagedFeedbackEndpoint,
}

impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let store = SqliteFeedbackStore::connect(&directory.path().join("scope.sqlite3")).await?;
        store
            .save_agent_config(AgentConfig {
                catalog_id: None,
                id: "desktop-fixture".into(),
                name: "Desktop entrypoint fixture".into(),
                host_id: "pi".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: "fixture-never-launched".into(),
                args: vec![],
                env: BTreeMap::new(),
                created_at: "2026-09-05T00:00:00Z".into(),
                updated_at: "2026-09-05T00:00:00Z".into(),
            })
            .await?;
        let session = store
            .create_managed_session(NewManagedSession {
                session_id: "desktop-entrypoint".into(),
                agent_config_id: "desktop-fixture".into(),
                cwd: directory.path().to_string_lossy().into_owned(),
                title: "Isolated entrypoint".into(),
                created_at: "2026-09-05T00:00:00Z".into(),
            })
            .await?;
        let app = store.clone().into_application();
        let provider = Arc::new(LocalManagedFeedbackProvider::new(app.clone()));
        let server = start_server_with_managed(
            ServerConfig::new(AccessToken::generate()).with_port(0),
            app,
            provider.clone(),
        )
        .await?;
        let endpoint = provider.bind(&session).await?;
        Ok(Self {
            directory,
            store,
            server,
            endpoint,
        })
    }

    fn command(&self) -> Command {
        let root = self.directory.path();
        let mut command = Command::new(env!("CARGO_BIN_EXE_rambledesk"));
        command
            .current_dir(root)
            .env(
                "RAMBLEDESK_DATABASE_FILE",
                root.join("must-not-open.sqlite3"),
            )
            .env(
                "RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE",
                root.join("must-not-create.token"),
            )
            .env(
                "RAMBLEDESK_LOCAL_SERVER_TOKEN",
                "unused-global-fixture-token",
            )
            .env("HOME", root)
            .env("USERPROFILE", root)
            .env("APPDATA", root.join("roaming"))
            .env("LOCALAPPDATA", root.join("local"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("RUST_LOG", "trace")
            .env(rambledesk_mcp::managed_stdio::URL_ENV, &self.endpoint.url)
            .env(
                rambledesk_mcp::managed_stdio::TOKEN_ENV,
                &self.endpoint.bearer_token,
            )
            .env_remove(rambledesk_acp::pi_wrapper::WRAPPER_ENV)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        command
    }

    async fn finish(self) -> anyhow::Result<()> {
        for path in [
            "must-not-open.sqlite3",
            "must-not-create.token",
            "local",
            "roaming",
        ] {
            ensure!(
                !self.directory.path().join(path).exists(),
                "early dispatch entered desktop setup"
            );
        }
        self.server.shutdown().await?;
        self.store.close().await;
        Ok(())
    }
}

struct DesktopProcess {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    errors: tokio::task::JoinHandle<std::io::Result<Vec<u8>>>,
    transcript: String,
}

impl DesktopProcess {
    fn spawn(mut command: Command) -> anyhow::Result<Self> {
        let mut child = command.spawn().context("spawn actual Desktop binary")?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().context("Desktop stdout")?);
        let mut stderr = child.stderr.take().context("Desktop stderr")?;
        let errors = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await?;
            Ok(bytes)
        });
        Ok(Self {
            child,
            input,
            output,
            errors,
            transcript: String::new(),
        })
    }

    async fn write(&mut self, message: Value) -> anyhow::Result<()> {
        let mut bytes = serde_json::to_vec(&message)?;
        bytes.push(b'\n');
        self.input
            .as_mut()
            .context("Desktop stdin")?
            .write_all(&bytes)
            .await?;
        Ok(())
    }

    async fn response(&mut self, id: u32) -> anyhow::Result<Value> {
        tokio::time::timeout(DEADLINE, async {
            loop {
                let mut line = String::new();
                ensure!(
                    self.output.read_line(&mut line).await? > 0,
                    "Desktop protocol closed"
                );
                let value: Value = serde_json::from_str(&line)
                    .context("Desktop stdout must contain protocol JSON only")?;
                ensure!(
                    value["jsonrpc"] == "2.0" || value["type"] == "response",
                    "Desktop stdout contains a non-protocol JSON log"
                );
                self.transcript.push_str(&line);
                if value["id"] == id {
                    return Ok(value);
                }
            }
        })
        .await
        .context("Desktop protocol deadline")?
    }

    async fn eof(mut self, endpoint: &ManagedFeedbackEndpoint) -> anyhow::Result<()> {
        drop(self.input.take());
        let status = tokio::time::timeout(DEADLINE, self.child.wait())
            .await
            .context("Desktop exits after stdin EOF")??;
        ensure!(status.success(), "Desktop entrypoint exit: {status}");
        let mut remainder = String::new();
        tokio::time::timeout(DEADLINE, self.output.read_to_string(&mut remainder)).await??;
        for line in remainder.lines() {
            let value: Value =
                serde_json::from_str(line).context("no stdout diagnostics on exit")?;
            ensure!(
                value["jsonrpc"] == "2.0" || value["type"] == "response",
                "Desktop stdout contains a non-protocol JSON log on exit"
            );
        }
        self.transcript.push_str(&remainder);
        let errors = tokio::time::timeout(DEADLINE, self.errors).await???;
        let errors = String::from_utf8_lossy(&errors);
        for secret in [&endpoint.bearer_token, "unused-global-fixture-token"] {
            ensure!(
                !self.transcript.contains(secret) && !errors.contains(secret),
                "credential in process output"
            );
        }
        ensure!(errors.len() < 1024, "unbounded Desktop diagnostics");
        Ok(())
    }
}

#[tokio::test]
async fn desktop_binary_dispatches_managed_stdio_before_application_setup() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let mut command = fixture.command();
    command.arg("managed-mcp-stdio");
    let mut desktop = DesktopProcess::spawn(command)?;
    desktop.write(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"desktop-fixture","version":"1"}
    }})).await?;
    let initialized = desktop.response(1).await?;
    ensure!(
        initialized["result"]["capabilities"]["tools"].is_object(),
        "managed MCP capability"
    );
    desktop
        .write(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    desktop
        .write(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await?;
    let listed = desktop.response(2).await?;
    let mut names = listed["result"]["tools"]
        .as_array()
        .context("tools")?
        .iter()
        .map(|tool| tool["name"].as_str().context("tool name"))
        .collect::<anyhow::Result<Vec<_>>>()?;
    names.sort_unstable();
    assert_eq!(
        names,
        ["get_feedback", "recover_feedback", "request_feedback"]
    );
    desktop.eof(&fixture.endpoint).await?;
    fixture.finish().await
}

#[tokio::test]
async fn desktop_binary_dispatches_pi_rpc_before_application_setup() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let extension = rambledesk_acp::pi_wrapper::install_managed_extension(
        &fixture.directory.path().join("extension"),
    )
    .await?;
    let node = rambledesk_core::find_executable("node").context("Node for protocol fixture")?;
    let native = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/desktop_pi_rpc.mjs");
    let mut command = fixture.command();
    command
        .args(["--mode", "rpc", "--no-themes"])
        .env(rambledesk_acp::pi_wrapper::WRAPPER_ENV, "1")
        .env(rambledesk_acp::pi_wrapper::COMMAND_ENV, node)
        .env(
            rambledesk_acp::pi_wrapper::ARGS_ENV,
            serde_json::to_string(&vec![native])?,
        )
        .env(rambledesk_acp::pi_wrapper::EXTENSION_ENV, extension)
        .env("PI_ACP_PI_COMMAND", env!("CARGO_BIN_EXE_rambledesk"));
    let mut desktop = DesktopProcess::spawn(command)?;
    desktop.write(json!({"id":1,"type":"get_state"})).await?;
    let state = desktop.response(1).await?;
    assert_eq!(state["type"], "response");
    assert_eq!(state["command"], "get_state");
    assert_eq!(state["success"], true);
    assert_eq!(
        state["data"]["tools"],
        json!(["get_feedback", "recover_feedback", "request_feedback"])
    );
    desktop.eof(&fixture.endpoint).await?;
    fixture.finish().await
}
