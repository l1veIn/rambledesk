//! Explicit opt-in integration probe. Credentials belong in inherited environment,
//! never in the launch JSON. See docs/ACP_BACKEND_PROBE.md for the invocation.
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, ensure};
use rambledesk_acp::AcpSessionDriver;
use rambledesk_core::*;
use rambledesk_local_server::{
    AccessToken, LocalManagedFeedbackProvider, ServerConfig, start_server_with_managed,
};
use rambledesk_storage::SqliteFeedbackStore;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Launch {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default = "default_label")]
    label: String,
}
fn default_label() -> String {
    "ACP backend".into()
}

struct Conversation {
    session_id: String,
    remote_id: String,
    request_id: String,
    marker: String,
}

fn remote(snapshot: &ManagedSessionSnapshot) -> anyhow::Result<String> {
    match &snapshot.session.management {
        SessionManagement::Managed {
            remote_session_id: Some(id),
            ..
        } => Ok(id.clone()),
        _ => anyhow::bail!("Agent session identity was not established"),
    }
}

async fn snapshot(app: &SessionApplication, id: &str) -> anyhow::Result<ManagedSessionSnapshot> {
    Ok(app
        .get_session(ManagedSessionInput {
            session_id: id.into(),
        })
        .await?)
}

async fn wait_for(
    app: &SessionApplication,
    id: &str,
    stage: &str,
    predicate: impl Fn(&ManagedSessionSnapshot) -> bool,
) -> anyhow::Result<ManagedSessionSnapshot> {
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let value = snapshot(app, id).await?;
            ensure!(
                value.permissions.is_empty(),
                "Probe stopped at an Agent permission request; no automatic approval was granted"
            );
            ensure!(
                value.runtime.connection == SessionConnectionState::Connected,
                "Agent disconnected during {stage}"
            );
            if predicate(&value) {
                return Ok(value);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .with_context(|| format!("Timed out during {stage}"))?
}

async fn run_conversations(
    app: &SessionApplication,
    feedback: &FeedbackApplication,
    store: &SqliteFeedbackStore,
    config: &str,
    root: &std::path::Path,
    report: &mut Value,
) -> anyhow::Result<Vec<Conversation>> {
    let first = app.create_session(CreateManagedSessionInput {
        agent_config_id: config.into(),
        cwd: root.join("project-a").to_string_lossy().into_owned(),
        title: "Managed probe A".into(),
    });
    let second = app.create_session(CreateManagedSessionInput {
        agent_config_id: config.into(),
        cwd: root.join("project-b").to_string_lossy().into_owned(),
        title: "Managed probe B".into(),
    });
    let (first, second) = tokio::join!(first, second);
    let mut conversations = vec![];
    for (index, value) in [first?, second?].into_iter().enumerate() {
        ensure!(
            value.runtime.connection == SessionConnectionState::Connected,
            "Agent session startup failed: {}",
            value
                .runtime
                .last_error
                .as_deref()
                .unwrap_or("no diagnostic")
        );
        conversations.push(Conversation {
            session_id: value.session.session_id.clone(),
            remote_id: remote(&value)?,
            request_id: uuid::Uuid::now_v7().to_string(),
            marker: format!("RD_MANAGED_{}_{}", index + 1, uuid::Uuid::now_v7().simple()),
        });
    }
    ensure!(
        conversations[0].remote_id != conversations[1].remote_id,
        "Two projects returned the same Agent session identity"
    );
    report["sessions"] = json!(conversations.iter().map(|item| json!({"session_id":item.session_id,"remote_session_id":item.remote_id,"request_id":item.request_id})).collect::<Vec<_>>());
    println!("{}", json!({"stage":"connected","sessions":2}));
    let prompts: Vec<_> = conversations.iter().map(|item| SendManagedPromptInput {
        session_id: item.session_id.clone(),
        text: format!("This is a harmless RambleDesk integration probe in an isolated project. Follow the built-in session workflow and execute the RambleDesk feedback request command exactly once with request_id '{}', title 'Managed integration probe', what_happened 'Please provide the probe marker', and actions [{{\"id\":\"marker\",\"instruction\":\"Provide the unique probe marker\"}}]. Use stdin or a temporary JSON file. Do not supply host/session identity. After the command returns, END THIS TURN immediately; do not wait, poll, read feedback yet, create another request, or ask for external confirmation. RambleDesk will later continue this SAME Agent session. On that continuation, execute feedback get with the original request_id, read the returned feedback package, and reply only with its exact marker. Only run commands needed for this feedback workflow. Do not edit project files or inspect credentials, configuration, or unrelated files.", item.request_id),
    }).collect();
    let (one, two) = tokio::join!(
        app.send_prompt(prompts[0].clone()),
        app.send_prompt(prompts[1].clone())
    );
    one?;
    two?;
    let requested = async |item: &Conversation| {
        tokio::time::timeout(Duration::from_secs(180), async {
            loop {
                let value = snapshot(app, &item.session_id).await?;
                ensure!(
                    value.permissions.is_empty(),
                    "Probe stopped at an Agent permission request"
                );
                match store.get_request(&item.request_id).await {
                    Ok(request) => {
                        ensure!(
                            request.managed_session_id.as_deref() == Some(item.session_id.as_str()),
                            "Feedback ownership mismatch"
                        );
                        if value.runtime.activity == SessionActivityState::Idle {
                            return Ok::<_, anyhow::Error>(());
                        }
                    }
                    Err(RepositoryError::RequestNotFound) => {}
                    Err(error) => return Err(error.into()),
                }
                ensure!(
                    value.runtime.connection == SessionConnectionState::Connected,
                    "Agent disconnected while requesting feedback"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .context("Timed out waiting for managed feedback and end of turn")?
    };
    let (one, two) = tokio::join!(requested(&conversations[0]), requested(&conversations[1]));
    one?;
    two?;
    println!("{}", json!({"stage":"feedback_waiting","sessions":2}));
    for item in &conversations {
        let draft = feedback.save_feedback_draft(SaveDraftInput {
            request_id: item.request_id.clone(), expected_revision: 0,
            document_json: r#"{"schemaVersion":2,"doc":{"type":"doc"}}"#.into(),
            body_markdown: format!("The unique marker is:\n\n{}\n\nRead this feedback markdown file, then reply only with that marker. Do not create another feedback request or edit files.", item.marker),
        }).await?;
        feedback
            .submit_feedback(SubmitFeedbackInput {
                request_id: item.request_id.clone(),
                expected_revision: draft.saved_revision,
                cooked_markdown: None,
                cooking_model: None,
                uncooked_markdown: None,
            })
            .await?;
    }
    println!("{}", json!({"stage":"feedback_submitted","sessions":2}));
    for (index, item) in conversations.iter().enumerate() {
        let done = wait_for(app, &item.session_id, "automatic continuation", |value| {
            value.deliveries.iter().any(|delivery| {
                delivery.request_id == item.request_id
                    && matches!(
                        delivery.state,
                        FeedbackDeliveryState::Delivered | FeedbackDeliveryState::Uncertain
                    )
            })
        })
        .await?;
        let delivery = done
            .deliveries
            .iter()
            .find(|delivery| delivery.request_id == item.request_id)
            .context("delivery record")?;
        report["sessions"][index]["delivery_state"] = serde_json::to_value(delivery.state)?;
        ensure!(
            delivery.state == FeedbackDeliveryState::Delivered,
            "Continuation outcome is uncertain; probe will not automatically retry it"
        );
        ensure!(
            remote(&done)? == item.remote_id,
            "Continuation changed the Agent session identity"
        );
        let messages = done
            .activities
            .iter()
            .filter(|row| row.kind == SessionActivityKind::AgentMessage)
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let own = messages.contains(&item.marker);
        let foreign = messages.contains(&conversations[1 - index].marker);
        report["sessions"][index]["own_marker_observed"] = json!(own);
        report["sessions"][index]["foreign_marker_observed"] = json!(foreign);
        report["sessions"][index]["observed_marker"] =
            if own { json!(item.marker) } else { Value::Null };
        report["sessions"][index]["tool_titles"] = json!(
            done.activities
                .iter()
                .filter(|row| row.kind == SessionActivityKind::ToolCall)
                .map(|row| row.text.clone())
                .collect::<Vec<_>>()
        );
        ensure!(
            own && !foreign,
            "Agent continuation did not return only its own feedback marker"
        );
    }
    println!(
        "{}",
        json!({"stage":"continued","sessions":2,"markers_matched":true})
    );
    for (index, item) in conversations.iter().enumerate() {
        app.stop_session(ManagedSessionInput {
            session_id: item.session_id.clone(),
        })
        .await?;
        let resumed = app
            .start_session(ManagedSessionInput {
                session_id: item.session_id.clone(),
            })
            .await?;
        ensure!(
            resumed.runtime.connection == SessionConnectionState::Connected,
            "Agent session recovery failed"
        );
        ensure!(
            remote(&resumed)? == item.remote_id,
            "Recovery replaced the original Agent session"
        );
        report["sessions"][index]["resumed_original_session"] = json!(true);
    }
    Ok(conversations)
}

async fn verify_reopened(
    root: &std::path::Path,
    conversations: &[Conversation],
    report: &mut Value,
) -> anyhow::Result<()> {
    let store = Arc::new(
        SqliteFeedbackStore::connect_with_library(
            &root.join("database.sqlite"),
            &root.join("library"),
        )
        .await?,
    );
    let feedback = store.as_ref().clone().into_application();
    let provider = Arc::new(LocalManagedFeedbackProvider::new(feedback.clone()));
    let server = start_server_with_managed(
        ServerConfig::new(AccessToken::generate()).with_port(0),
        feedback,
        provider.clone(),
    )
    .await?;
    let app = SessionApplication::new(
        store.clone(),
        store.clone(),
        Arc::new(AcpSessionDriver::with_feedback_companion(
            std::env::current_exe()?,
        )),
    )
    .with_feedback_provider(provider)
    .with_deliveries(store.clone())
    .with_deletions(store.clone())
    .with_recovery(store.clone());
    let result = async {
        app.recover_runtime().await?;
        app.start_delivery_worker().await?;
        for (index, item) in conversations.iter().enumerate() {
            let recovered = snapshot(&app, &item.session_id).await?;
            ensure!(recovered.runtime.connection == SessionConnectionState::Stopped
                && recovered.runtime.instance_id.is_none(), "Reopening implicitly launched an Agent");
            ensure!(remote(&recovered)? == item.remote_id, "Reopening changed Agent identity");
            report["sessions"][index]["reopened_without_launch"] = json!(true);
        }
        let restore = async |item: &Conversation| {
            let resumed = app.start_session(ManagedSessionInput { session_id: item.session_id.clone() }).await?;
            ensure!(resumed.runtime.connection == SessionConnectionState::Connected
                && remote(&resumed)? == item.remote_id, "Full-runtime recovery failed");
            let boundary = resumed.activities.iter().map(|row| row.sequence).max().unwrap_or(0);
            app.send_prompt(SendManagedPromptInput {
                session_id: item.session_id.clone(),
                text: format!("RambleDesk was restarted and explicitly resumed this SAME Agent conversation. Execute the built-in feedback get command now for the existing request_id '{}', read the returned feedback package again, then reply only with its exact unique marker. Use the current inherited command capability; do not rely only on remembered text. Do not create a new feedback request or edit files. Only run commands needed to read this feedback. Do not inspect credentials, config, or unrelated files.",item.request_id),
            }).await?;
            let done = wait_for(&app, &item.session_id, "reopened feedback read", |value| {
                value.runtime.activity == SessionActivityState::Idle
                    && value.activities.iter().any(|row| row.sequence > boundary && row.kind == SessionActivityKind::Status && row.text.starts_with("Turn finished:"))
            }).await?;
            ensure!(remote(&done)? == item.remote_id, "Reopened prompt changed identity");
            let new_rows: Vec<_> = done.activities.iter().filter(|row| row.sequence > boundary).collect();
            ensure!(new_rows.iter().any(|row| row.kind == SessionActivityKind::ToolCall), "Reopened Agent did not execute a tool to read feedback");
            let messages = new_rows.iter().filter(|row| row.kind == SessionActivityKind::AgentMessage)
                .map(|row| row.text.as_str()).collect::<Vec<_>>().join("\n");
            ensure!(messages.contains(&item.marker), "Reopened Agent did not read its original marker");
            ensure!(!conversations.iter().any(|other| other.session_id != item.session_id && messages.contains(&other.marker)), "Reopened Agent returned another session's marker");
            Ok::<_, anyhow::Error>(new_rows.iter().filter(|row| row.kind == SessionActivityKind::ToolCall).map(|row| row.text.clone()).collect::<Vec<_>>())
        };
        let (one, two) = tokio::join!(restore(&conversations[0]), restore(&conversations[1]));
        for (index, tools) in [one?, two?].into_iter().enumerate() {
            report["sessions"][index]["full_runtime_resumed_original"] = json!(true);
            report["sessions"][index]["reopened_marker_observed"] = json!(true);
            report["sessions"][index]["reopened_tool_titles"] = json!(tools);
        }
        println!("{}", json!({"stage":"reopened_continued","sessions":2,"markers_matched":true}));
        app.delete_managed_session(ManagedSessionInput { session_id: conversations[0].session_id.clone() }).await?;
        ensure!(matches!(store.get_session(&conversations[0].session_id).await, Err(SessionRepositoryError::SessionNotFound)), "Deleted session persisted");
        ensure!(matches!(store.get_request(&conversations[0].request_id).await, Err(RepositoryError::RequestNotFound)), "Deleted request persisted");
        ensure!(store.list_session_deliveries(&conversations[0].session_id).await?.is_empty(), "Deleted delivery persisted");
        let neighbor = snapshot(&app, &conversations[1].session_id).await?;
        ensure!(neighbor.runtime.connection == SessionConnectionState::Connected && remote(&neighbor)? == conversations[1].remote_id, "Deleting one session affected its neighbor");
        report["delete_removed_session_request_delivery"] = json!(true);
        report["delete_preserved_neighbor_connection"] = json!(true);
        Ok::<_, anyhow::Error>(())
    }.await;
    let app_cleanup = app.shutdown().await;
    let server_cleanup = server.shutdown().await;
    store.close().await;
    report["reopened_cleanup_complete"] = json!(app_cleanup.is_ok() && server_cleanup.is_ok());
    app_cleanup?;
    server_cleanup?;
    result
}

fn main() -> anyhow::Result<()> {
    if rambledesk_feedback_client::process_requested() {
        std::process::exit(rambledesk_feedback_client::run_process());
    }
    run_probe()
}

#[tokio::main]
async fn run_probe() -> anyhow::Result<()> {
    if std::env::var("RAMBLEDESK_MANAGED_PROBE_RUN").as_deref() != Ok("1") {
        println!(
            "No Agent launched. Set RAMBLEDESK_MANAGED_PROBE_RUN=1, RAMBLEDESK_MANAGED_PROBE_LAUNCH=<launch.json>, and RAMBLEDESK_MANAGED_PROBE_RUN_DIR=<new absolute directory> to run a real two-project feedback probe. Model usage may be billed."
        );
        return Ok(());
    }
    let launch_path =
        std::env::var("RAMBLEDESK_MANAGED_PROBE_LAUNCH").context("launch path required")?;
    let root = PathBuf::from(
        std::env::var("RAMBLEDESK_MANAGED_PROBE_RUN_DIR").context("run directory required")?,
    );
    ensure!(root.is_absolute(), "Probe run directory must be absolute");
    ensure!(
        !root.join("database.sqlite").exists(),
        "Probe requires a fresh database path"
    );
    let launch: Launch = serde_json::from_slice(&tokio::fs::read(launch_path).await?)?;
    for project in ["project-a", "project-b"] {
        tokio::fs::create_dir_all(root.join(project)).await?;
    }
    let store = Arc::new(
        SqliteFeedbackStore::connect_with_library(
            &root.join("database.sqlite"),
            &root.join("library"),
        )
        .await?,
    );
    let feedback = store.as_ref().clone().into_application();
    let provider = Arc::new(LocalManagedFeedbackProvider::new(feedback.clone()));
    let server = start_server_with_managed(
        ServerConfig::new(AccessToken::generate()).with_port(0),
        feedback.clone(),
        provider.clone(),
    )
    .await?;
    let app = SessionApplication::new(
        store.clone(),
        store.clone(),
        Arc::new(AcpSessionDriver::with_feedback_companion(
            std::env::current_exe()?,
        )),
    )
    .with_feedback_provider(provider)
    .with_deliveries(store.clone())
    .with_deletions(store.clone())
    .with_recovery(store.clone());
    let mut report = json!({"label":launch.label,"sessions":[],"cleanup_complete":false});
    let result = async {
        app.start_delivery_worker().await?;
        let config = app
            .save_agent_config(SaveAgentConfigInput {
                catalog_id: None,
                id: None,
                name: "Isolated managed probe".into(),
                host_id: "dsh".into(),
                protocol: SessionProtocol::Acp,
                enabled: true,
                command: launch.command,
                args: launch.args,
                env: launch.env,
            })
            .await?;
        run_conversations(&app, &feedback, &store, &config.id, &root, &mut report).await
    }
    .await;
    let app_cleanup = app.shutdown().await;
    let server_cleanup = server.shutdown().await;
    store.close().await;
    report["cleanup_complete"] = json!(app_cleanup.is_ok() && server_cleanup.is_ok());
    let result = match result {
        Ok(conversations) if app_cleanup.is_ok() && server_cleanup.is_ok() => {
            verify_reopened(&root, &conversations, &mut report).await
        }
        Ok(_) => Err(anyhow::anyhow!("Initial runtime cleanup failed")),
        Err(error) => Err(error),
    };
    report["success"] = json!(result.is_ok());
    if let Err(error) = &result {
        report["error"] = json!(error.to_string());
    }
    tokio::fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )
    .await?;
    println!(
        "{}",
        json!({"stage":"finished","success":result.is_ok(),"cleanup_complete":report["cleanup_complete"],"report":root.join("report.json")})
    );
    app_cleanup?;
    server_cleanup?;
    result
}
