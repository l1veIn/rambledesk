use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use rambledesk_local_server::{
    AccessToken, DEFAULT_PORT, ServerConfig, default_token_path, start_server,
};
use rmcp::{
    ServiceExt,
    model::ClientInfo,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};

#[derive(Debug, Parser)]
#[command(name = "rambledesk", version, about = "RambleDesk local server")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Forward private instance feedback MCP over stdio; credentials come only from environment.
    ManagedMcpStdio,
    /// Run the authenticated loopback local server without Tauri.
    Serve {
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long)]
        token_file: Option<PathBuf>,
        #[arg(long)]
        database_file: Option<PathBuf>,
        #[arg(long)]
        print_token: bool,
    },
    /// Connect with the official Rust SDK and list Generic MCP Adapter tools.
    Smoke {
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        token_file: Option<PathBuf>,
    },
    /// Start an ephemeral server and run the SDK smoke test end to end.
    SelfTest,
}

fn main() -> anyhow::Result<()> {
    if rambledesk_acp::pi_wrapper::process_requested() {
        std::process::exit(rambledesk_acp::pi_wrapper::run_process());
    }
    // No tracing subscriber in companion mode, even when RUST_LOG=trace: SDK
    // diagnostics can include capability headers and model-supplied tool data.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "managed-mcp-stdio")
    {
        std::process::exit(rambledesk_mcp::managed_stdio::run_process());
    }
    run_cli()
}

#[tokio::main]
async fn run_cli() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rambledesk=info".into()),
        )
        .with_target(false)
        .init();

    match arguments.command {
        Command::ManagedMcpStdio => unreachable!("handled before logging initialization"),
        Command::Serve {
            port,
            token_file,
            database_file,
            print_token,
        } => {
            let token_file = token_file.unwrap_or(default_token_path()?);
            let database_file =
                database_file.unwrap_or(rambledesk_storage::default_database_path()?);
            let token = AccessToken::load_or_create(&token_file)?;
            let store = rambledesk_storage::SqliteFeedbackStore::connect(&database_file).await?;
            let application = store.clone().into_application();
            let server = start_server(
                ServerConfig::new(token.clone()).with_port(port),
                application,
            )
            .await?;
            let mut status = serde_json::json!({
                "endpoint": server.endpoint(),
                "tokenFile": token_file,
                "databaseFile": database_file,
                "authorizationHeader": "Bearer <token>",
                "protocolCandidates": ["2026-07-28", "2025-11-25"]
            });
            if print_token {
                status["accessToken"] = serde_json::Value::String(token.secret().to_owned());
            }
            println!("{}", serde_json::to_string_pretty(&status)?);
            tokio::signal::ctrl_c().await?;
            server.shutdown().await?;
            store.close().await;
        }
        Command::Smoke {
            endpoint,
            token,
            token_file,
        } => {
            let token = match token {
                Some(token) => AccessToken::parse(token)?,
                None => {
                    let path = token_file.unwrap_or(default_token_path()?);
                    AccessToken::load_or_create(&path)?
                }
            };
            let result = smoke(&endpoint, &token).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::SelfTest => {
            let directory = tempfile::tempdir()?;
            let store = rambledesk_storage::SqliteFeedbackStore::connect(
                &directory.path().join("rambledesk.sqlite3"),
            )
            .await?;
            let token = AccessToken::generate();
            let application = store.clone().into_application();
            let server =
                start_server(ServerConfig::new(token.clone()).with_port(0), application).await?;
            let result = smoke(server.endpoint(), &token).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            server.shutdown().await?;
            store.close().await;
        }
    }

    Ok(())
}

async fn smoke(endpoint: &str, token: &AccessToken) -> anyhow::Result<serde_json::Value> {
    let config = StreamableHttpClientTransportConfig::with_uri(endpoint.to_owned())
        .auth_header(token.secret());
    let transport = StreamableHttpClientTransport::from_config(config);
    let client = ClientInfo::default()
        .serve(transport)
        .await
        .context("initialize MCP client")?;

    let tools = client
        .peer()
        .list_tools(Default::default())
        .await
        .context("list MCP tools")?;
    let tool_names: Vec<String> = tools
        .tools
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();

    let expected = ["request_feedback", "get_feedback", "cancel_feedback"];
    let ok = expected
        .iter()
        .all(|name| tool_names.iter().any(|tool| tool == name))
        && tool_names.len() == expected.len();
    client.cancel().await?;

    Ok(serde_json::json!({
        "endpoint": endpoint,
        "tools": tool_names,
        "ok": ok
    }))
}
