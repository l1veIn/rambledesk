//! Usage: cargo run -p rambledesk-acp --example smoke -- launch.json [prompt]
//! Launch JSON: {"command":"deepseek-acp","args":[],"cwd":"C:/project"}.
//! The optional third argument is an existing remote session ID to load.
use rambledesk_acp::{AcpConnection, AcpEvent, AcpLaunch};
use std::{sync::Arc, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("provide a launch JSON file")?;
    let prompt = args.next();
    let remote = args.next();
    let launch: AcpLaunch = serde_json::from_slice(&std::fs::read(path)?)?;
    let connection = AcpConnection::connect(
        &launch,
        Arc::new(|event| match event {
            AcpEvent::Update(notification) => {
                use agent_client_protocol::schema::v1::{ContentBlock, SessionUpdate};
                if let SessionUpdate::AgentMessageChunk(chunk) = notification.update
                    && let ContentBlock::Text(content) = chunk.content
                {
                    print!("{}", content.text);
                }
            }
            AcpEvent::PermissionDeclined => {
                eprintln!("permission declined (probe has no approval UI)")
            }
            AcpEvent::PermissionRequested { .. } => {
                unreachable!("probe permissions are always declined")
            }
        }),
    )
    .await?;
    let outcome = async {
        let info = tokio::time::timeout(
            Duration::from_secs(60),
            connection.open_session(&launch, remote.as_deref()),
        )
        .await??;
        println!("{}", serde_json::to_string_pretty(&info)?);
        if let Some(prompt) = prompt {
            println!(
                "stop_reason={}",
                tokio::time::timeout(
                    Duration::from_secs(180),
                    connection.prompt(&info.remote_session_id, &prompt)
                )
                .await??
            );
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    }
    .await;
    connection.shutdown().await?;
    outcome
}
