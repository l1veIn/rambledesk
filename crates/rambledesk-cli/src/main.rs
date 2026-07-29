use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};
use rambledesk_mcp::{AccessToken, ServerConfig, default_token_path, start_server};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientInfo},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};

#[derive(Debug, Parser)]
#[command(name = "rambledesk", version, about = "RambleDesk M0 diagnostics")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the authenticated loopback MCP server without Tauri.
    Serve {
        #[arg(long, default_value_t = rambledesk_mcp::DEFAULT_PORT)]
        port: u16,
        #[arg(long)]
        token_file: Option<PathBuf>,
        #[arg(long)]
        print_token: bool,
    },
    /// Connect with the official Rust SDK and call rambledesk_health.
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rambledesk=info".into()),
        )
        .with_target(false)
        .init();

    match Arguments::parse().command {
        Command::Serve {
            port,
            token_file,
            print_token,
        } => {
            let token_file = token_file.unwrap_or(default_token_path()?);
            let token = AccessToken::load_or_create(&token_file)?;
            let server = start_server(ServerConfig::new(token.clone()).with_port(port)).await?;
            let mut status = serde_json::json!({
                "endpoint": server.endpoint(),
                "tokenFile": token_file,
                "authorizationHeader": "Bearer <token>",
                "protocolCandidates": ["2026-07-28", "2025-11-25"]
            });
            if print_token {
                status["accessToken"] = serde_json::Value::String(token.secret().to_owned());
            }
            println!("{}", serde_json::to_string_pretty(&status)?);
            tokio::signal::ctrl_c().await?;
            server.shutdown().await?;
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
            let token = AccessToken::generate();
            let server = start_server(ServerConfig::new(token.clone()).with_port(0)).await?;
            let result = smoke(server.endpoint(), &token).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            server.shutdown().await?;
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

    let health = client
        .call_tool(
            CallToolRequestParams::new("rambledesk_health")
                .with_arguments(HashMap::new().into_iter().collect()),
        )
        .await
        .context("call rambledesk_health")?;
    let structured = health.structured_content.clone();
    client.cancel().await?;

    Ok(serde_json::json!({
        "endpoint": endpoint,
        "tools": tool_names,
        "health": structured,
        "ok": health.is_error != Some(true)
    }))
}
