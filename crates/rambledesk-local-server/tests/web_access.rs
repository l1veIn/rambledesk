use std::{net::SocketAddr, sync::Arc};

use futures::StreamExt;
use rambledesk_core::{
    ActionInput, ApplicationChangeHub, ApplicationCommandFacade, ApplicationEvent,
    ApplicationHostProfileView, ContextRef, FeedbackApplication, RequestFeedbackInput,
    SaveDraftInput, WorkbenchTerminalOperations,
};
use rambledesk_local_server::{
    EVENT_CREDENTIAL_PROTOCOL_PREFIX, EVENT_PROTOCOL, REVISION_HEADER, RUNTIME_GENERATION_HEADER,
    WebAccessRouteConfig, WebSessionAuthenticator, web_access_router,
};
use rambledesk_storage::SqliteFeedbackStore;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WebSocketError, Message, client::IntoClientRequest},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const SESSION_TOKEN: &str = "test_session_token";
const RUNTIME_GENERATION: &str = "runtime-test";

struct TestSessionAuthenticator;

impl WebSessionAuthenticator for TestSessionAuthenticator {
    fn authenticate(&self, session_token: &str) -> bool {
        session_token == SESSION_TOKEN
    }
}

struct WebAccessFixture {
    address: SocketAddr,
    application: FeedbackApplication,
    changes: Arc<ApplicationChangeHub>,
    cancellation: CancellationToken,
    task: JoinHandle<std::io::Result<()>>,
    _directory: tempfile::TempDir,
}

impl WebAccessFixture {
    async fn start() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let store = SqliteFeedbackStore::connect(&directory.path().join("state.sqlite3")).await?;
        let changes = Arc::new(ApplicationChangeHub::with_runtime_generation(
            RUNTIME_GENERATION,
        ));
        let application = store
            .into_application()
            .with_change_observer(changes.clone());
        let commands = Arc::new(ApplicationCommandFacade::new(
            application.clone(),
            WorkbenchTerminalOperations::without_observer(application.clone()),
            vec![ApplicationHostProfileView {
                id: "codex".into(),
                label: "Codex".into(),
                icon_svg: "<svg />".into(),
                default_adapter: "generic_mcp".into(),
                continuation_mode: "manual".into(),
            }],
        ));
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let authority = address.to_string();
        let router = axum::Router::new().nest(
            "/api",
            web_access_router(
                commands,
                changes.clone(),
                Arc::new(TestSessionAuthenticator),
                WebAccessRouteConfig::new(authority.clone(), format!("http://{authority}"), 2)
                    .map_err(anyhow::Error::msg)?,
            ),
        );
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move { task_cancellation.cancelled_owned().await })
                .await
        });
        Ok(Self {
            address,
            application,
            changes,
            cancellation,
            task,
            _directory: directory,
        })
    }

    fn http_url(&self, path: &str) -> String {
        format!("http://{}/api/{path}", self.address)
    }

    fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    async fn seed_request(&self) -> String {
        let request_id = Uuid::now_v7().to_string();
        self.application
            .request_feedback(RequestFeedbackInput {
                request_id: Some(request_id.clone()),
                host_id: Some("codex".into()),
                host_session_id: "web-events".into(),
                title: Some("Web events".into()),
                what_happened: "Exercise the Web Access event contract.".into(),
                actions: vec![ActionInput {
                    id: "verify".into(),
                    instruction: "Verify events.".into(),
                }],
                context_refs: vec![ContextRef {
                    label: "spec".into(),
                    uri: "file:///tmp/spec.md".into(),
                }],
                attachments: vec![],
                source_hint: Some("web access test".into()),
                allow_finish: false,
                final_summary: None,
            })
            .await
            .expect("seed request");
        request_id
    }

    async fn shutdown(self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        self.task.await??;
        Ok(())
    }
}

fn authorized(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: String,
    origin: &str,
) -> reqwest::RequestBuilder {
    client
        .request(method, url)
        .bearer_auth(SESSION_TOKEN)
        .header(reqwest::header::ORIGIN, origin)
}

async fn connect_event_socket(
    fixture: &WebAccessFixture,
) -> anyhow::Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
    let mut request = format!("ws://{}/api/events", fixture.address).into_client_request()?;
    request
        .headers_mut()
        .insert("Origin", fixture.origin().parse()?);
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("{EVENT_PROTOCOL}, {EVENT_CREDENTIAL_PROTOCOL_PREFIX}{SESSION_TOKEN}").parse()?,
    );
    let (socket, response) = connect_async(request).await?;
    assert_eq!(response.headers()["Sec-WebSocket-Protocol"], EVENT_PROTOCOL);
    Ok(socket)
}

async fn next_application_event(
    socket: &mut WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
) -> anyhow::Result<ApplicationEvent> {
    let frame = socket.next().await.expect("application event frame")?;
    let Message::Text(payload) = frame else {
        anyhow::bail!("expected application event text frame")
    };
    Ok(serde_json::from_str(&payload)?)
}

async fn rejected_event_socket_status(
    fixture: &WebAccessFixture,
    protocols: Option<&str>,
) -> anyhow::Result<u16> {
    let mut request = format!("ws://{}/api/events", fixture.address).into_client_request()?;
    request
        .headers_mut()
        .insert("Origin", fixture.origin().parse()?);
    if let Some(protocols) = protocols {
        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", protocols.parse()?);
    }
    match connect_async(request).await {
        Err(WebSocketError::Http(response)) => Ok(response.status().as_u16()),
        Err(error) => Err(error.into()),
        Ok(_) => anyhow::bail!("rejected WebSocket credentials unexpectedly connected"),
    }
}

#[tokio::test]
async fn web_access_rejects_an_inexact_host_before_authentication() -> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    let response = reqwest::Client::new()
        .post(fixture.http_url("application/listFeedbackInbox"))
        .header(reqwest::header::HOST, "attacker.example")
        .header(reqwest::header::ORIGIN, fixture.origin())
        .bearer_auth(SESSION_TOKEN)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    fixture.shutdown().await
}

#[tokio::test]
async fn web_access_rejects_an_inexact_origin_before_authentication() -> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    let response = reqwest::Client::new()
        .post(fixture.http_url("application/listFeedbackInbox"))
        .header(reqwest::header::ORIGIN, "http://attacker.example")
        .bearer_auth(SESSION_TOKEN)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    fixture.shutdown().await
}

#[tokio::test]
async fn same_origin_application_post_requires_the_exact_bearer() -> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    let client = reqwest::Client::new();
    for bearer in [None, Some("wrong-session-token")] {
        let mut request = client
            .post(fixture.http_url("application/listFeedbackInbox"))
            .header(reqwest::header::ORIGIN, fixture.origin());
        if let Some(bearer) = bearer {
            request = request.bearer_auth(bearer);
        }
        assert_eq!(
            request.send().await?.status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
    }
    fixture.shutdown().await
}

#[tokio::test]
async fn event_websocket_requires_the_exact_credential_protocol() -> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    assert_eq!(
        rejected_event_socket_status(&fixture, Some(EVENT_PROTOCOL)).await?,
        reqwest::StatusCode::UNAUTHORIZED.as_u16()
    );
    assert_eq!(
        rejected_event_socket_status(
            &fixture,
            Some(&format!(
                "{EVENT_PROTOCOL}, {EVENT_CREDENTIAL_PROTOCOL_PREFIX}wrong-session-token"
            )),
        )
        .await?,
        reqwest::StatusCode::UNAUTHORIZED.as_u16()
    );
    fixture.shutdown().await
}

#[tokio::test]
async fn health_and_application_snapshots_require_the_injected_session() -> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    let client = reqwest::Client::new();
    let wrong_method = authorized(
        &client,
        reqwest::Method::GET,
        fixture.http_url("health"),
        &fixture.origin(),
    )
    .send()
    .await?;
    assert_eq!(
        wrong_method.status(),
        reqwest::StatusCode::METHOD_NOT_ALLOWED
    );
    let health = authorized(
        &client,
        reqwest::Method::POST,
        fixture.http_url("health"),
        &fixture.origin(),
    )
    .send()
    .await?;
    assert_eq!(health.status(), reqwest::StatusCode::OK);
    assert_eq!(
        health.json::<serde_json::Value>().await?["runtime_generation"],
        RUNTIME_GENERATION
    );

    let response = authorized(
        &client,
        reqwest::Method::POST,
        fixture.http_url("application/listFeedbackInbox"),
        &fixture.origin(),
    )
    .send()
    .await?;
    assert_eq!(
        response.headers()[RUNTIME_GENERATION_HEADER],
        RUNTIME_GENERATION
    );
    assert_eq!(response.headers()[REVISION_HEADER], "0");
    fixture.shutdown().await
}

#[tokio::test]
async fn websocket_sends_ready_first_and_never_echoes_the_credential_protocol() -> anyhow::Result<()>
{
    let fixture = WebAccessFixture::start().await?;
    let mut socket = connect_event_socket(&fixture).await?;
    let ready = next_application_event(&mut socket).await?;
    assert!(matches!(ready, ApplicationEvent::Ready { revision, .. } if revision == "0"));

    let request_id = fixture.seed_request().await;
    let invalidation = next_application_event(&mut socket).await?;
    assert!(matches!(
        invalidation,
        ApplicationEvent::Invalidate { revision, resources, .. }
            if revision == "1" && resources.iter().any(|resource| matches!(
                resource,
                rambledesk_core::ApplicationResourceKey::FeedbackWorkspace { request_id: id }
                    if id == &request_id
            ))
    ));
    socket.close(None).await?;
    fixture.shutdown().await
}

#[tokio::test]
async fn two_authenticated_clients_observe_the_same_committed_revision() -> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    let mut first = connect_event_socket(&fixture).await?;
    let mut second = connect_event_socket(&fixture).await?;
    assert!(matches!(
        next_application_event(&mut first).await?,
        ApplicationEvent::Ready { revision, .. } if revision == "0"
    ));
    assert!(matches!(
        next_application_event(&mut second).await?,
        ApplicationEvent::Ready { revision, .. } if revision == "0"
    ));

    let request_id = fixture.seed_request().await;
    let first_event = next_application_event(&mut first).await?;
    let second_event = next_application_event(&mut second).await?;
    assert_eq!(first_event, second_event);
    assert!(matches!(
        first_event,
        ApplicationEvent::Invalidate { revision, resources, .. }
            if revision == "1" && resources.iter().any(|resource| matches!(
                resource,
                rambledesk_core::ApplicationResourceKey::FeedbackWorkspace { request_id: id }
                    if id == &request_id
            ))
    ));

    fixture
        .application
        .save_feedback_draft(SaveDraftInput {
            request_id: request_id.clone(),
            document_json: r#"{"type":"doc","content":[]}"#.into(),
            body_markdown: "Saved from the other client.".into(),
            expected_revision: 0,
        })
        .await?;
    let first_draft_event = next_application_event(&mut first).await?;
    let second_draft_event = next_application_event(&mut second).await?;
    assert_eq!(first_draft_event, second_draft_event);
    assert!(matches!(
        first_draft_event,
        ApplicationEvent::Invalidate { revision, resources, .. }
            if revision == "2"
                && resources == vec![
                    rambledesk_core::ApplicationResourceKey::Navigation,
                    rambledesk_core::ApplicationResourceKey::FeedbackWorkspace {
                        request_id: request_id.clone(),
                    },
                ]
    ));

    first.close(None).await?;
    second.close(None).await?;
    fixture.shutdown().await
}

#[tokio::test]
async fn stale_generation_rejects_mutation_without_side_effects() -> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    let request_id = fixture.seed_request().await;
    let before = fixture.changes.metadata();
    let client = reqwest::Client::new();
    let response = authorized(
        &client,
        reqwest::Method::POST,
        fixture.http_url("application/saveFeedbackDraft"),
        &fixture.origin(),
    )
    .header(RUNTIME_GENERATION_HEADER, "stale-runtime")
    .json(&SaveDraftInput {
        request_id: request_id.clone(),
        document_json: r#"{"type":"doc","content":[]}"#.into(),
        body_markdown: "must not save".into(),
        expected_revision: 0,
    })
    .send()
    .await?;
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        response.json::<serde_json::Value>().await?["code"],
        "RUNTIME_GENERATION_STALE"
    );
    assert_eq!(fixture.changes.metadata(), before);
    let workspace = fixture
        .application
        .get_feedback_workspace(request_id)
        .await?;
    assert_eq!(workspace.draft.saved_revision, 0);
    assert!(workspace.draft.body_markdown.is_empty());
    fixture.shutdown().await
}

#[tokio::test]
async fn successful_mutation_returns_the_advanced_runtime_ledger() -> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    let request_id = fixture.seed_request().await;
    let client = reqwest::Client::new();
    let response = authorized(
        &client,
        reqwest::Method::POST,
        fixture.http_url("application/saveFeedbackDraft"),
        &fixture.origin(),
    )
    .header(RUNTIME_GENERATION_HEADER, RUNTIME_GENERATION)
    .json(&SaveDraftInput {
        request_id: request_id.clone(),
        document_json: r#"{"type":"doc","content":[]}"#.into(),
        body_markdown: "saved".into(),
        expected_revision: 0,
    })
    .send()
    .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()[REVISION_HEADER], "2");
    let workspace = fixture
        .application
        .get_feedback_workspace(request_id)
        .await?;
    assert_eq!(workspace.draft.body_markdown, "saved");
    fixture.shutdown().await
}

#[tokio::test]
async fn concurrent_http_snapshot_never_labels_an_old_projection_with_a_new_revision()
-> anyhow::Result<()> {
    let fixture = WebAccessFixture::start().await?;
    let client = reqwest::Client::new();
    for _ in 0..16 {
        let request_id = Uuid::now_v7().to_string();
        let query = authorized(
            &client,
            reqwest::Method::POST,
            fixture.http_url("application/listFeedbackInbox"),
            &fixture.origin(),
        )
        .send();
        let create = fixture
            .application
            .request_feedback(fixture_request(request_id.clone()));
        let (response, created) = tokio::join!(query, create);
        created.expect("concurrent request creation");
        let response = response?;
        let snapshot_revision = response.headers()[REVISION_HEADER]
            .to_str()?
            .parse::<u64>()?;
        let requests = response
            .json::<Vec<rambledesk_core::FeedbackRequestSummary>>()
            .await?;
        let creation_revision = fixture.changes.metadata().revision.parse::<u64>()?;
        if snapshot_revision >= creation_revision {
            assert!(
                requests
                    .iter()
                    .any(|request| request.request_id == request_id),
                "a snapshot carrying the creation revision must contain the created request"
            );
        }
    }
    fixture.shutdown().await
}

fn fixture_request(request_id: String) -> RequestFeedbackInput {
    RequestFeedbackInput {
        request_id: Some(request_id),
        host_id: Some("codex".into()),
        host_session_id: "snapshot-race".into(),
        title: Some("Snapshot race".into()),
        what_happened: "Race a query with a mutation.".into(),
        actions: vec![ActionInput {
            id: "verify".into(),
            instruction: "Verify snapshot metadata.".into(),
        }],
        context_refs: vec![],
        attachments: vec![],
        source_hint: Some("snapshot race test".into()),
        allow_finish: false,
        final_summary: None,
    }
}

#[tokio::test]
async fn local_integration_listener_still_does_not_expose_web_access_routes() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = SqliteFeedbackStore::connect(&directory.path().join("local.sqlite3")).await?;
    let server = rambledesk_local_server::start_server(
        rambledesk_local_server::ServerConfig::new(rambledesk_local_server::AccessToken::parse(
            "a".repeat(64),
        )?)
        .with_port(0),
        store.into_application(),
    )
    .await?;
    let client = reqwest::Client::new();
    let local_health = client
        .get(format!("http://{}/api/health", server.address()))
        .bearer_auth("a".repeat(64))
        .send()
        .await?;
    assert_eq!(local_health.status(), reqwest::StatusCode::OK);
    assert!(
        local_health
            .headers()
            .get(RUNTIME_GENERATION_HEADER)
            .is_none(),
        "the existing Local Integration health route must not become Web Access health"
    );
    for path in ["events", "application/listFeedbackInbox"] {
        let response = client
            .get(format!("http://{}/api/{path}", server.address()))
            .bearer_auth("a".repeat(64))
            .send()
            .await?;
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND, "{path}");
    }
    server.shutdown().await?;
    Ok(())
}
