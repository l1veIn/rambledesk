use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anyhow::Context;
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

const GLOBAL_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct Fixture {
    _directory: tempfile::TempDir,
    store: SqliteFeedbackStore,
    server: ServerHandle,
    provider: Arc<LocalManagedFeedbackProvider>,
    sessions: Vec<SessionRecord>,
    client: reqwest::Client,
}

impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let store = SqliteFeedbackStore::connect(&directory.path().join("test.sqlite3")).await?;
        store
            .save_agent_config(AgentConfig {
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
        let application = store.clone().into_application();
        let provider = Arc::new(LocalManagedFeedbackProvider::new(application.clone()));
        assert!(provider.bind(&sessions[0]).await.is_err());
        let server = start_server_with_managed(
            ServerConfig::new(AccessToken::parse(GLOBAL_TOKEN)?).with_port(0),
            application,
            provider.clone(),
        )
        .await?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            _directory: directory,
            store,
            server,
            provider,
            sessions,
            client,
        })
    }

    fn post(
        &self,
        endpoint: &ManagedFeedbackEndpoint,
        session: Option<&str>,
        body: Value,
    ) -> reqwest::RequestBuilder {
        let request = self
            .client
            .post(&endpoint.url)
            .bearer_auth(&endpoint.bearer_token)
            .header("Accept", "application/json, text/event-stream")
            .json(&body);
        match session {
            Some(session) => request.header("Mcp-Session-Id", session),
            None => request,
        }
    }

    async fn initialize(&self, endpoint: &ManagedFeedbackEndpoint) -> anyhow::Result<String> {
        assert_eq!(
            endpoint.url,
            format!("http://{}/mcp-managed", self.server.address())
        );
        assert!(!endpoint.url.contains(&endpoint.bearer_token));
        let response = self.post(endpoint, None, json!({
            "jsonrpc":"2.0", "id":1, "method":"initialize", "params":{
                "protocolVersion":"2025-03-26", "capabilities":{}, "clientInfo":{"name":"fixture","version":"1"}
            }
        })).send().await?.error_for_status()?;
        let session = response
            .headers()
            .get("Mcp-Session-Id")
            .context("session id")?
            .to_str()?
            .to_owned();
        let result = rpc_body(response).await?;
        assert!(
            result["result"]["instructions"]
                .as_str()
                .context("instructions")?
                .contains("end the current Agent turn")
        );
        self.post(
            endpoint,
            Some(&session),
            json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
        )
        .send()
        .await?
        .error_for_status()?;
        Ok(session)
    }

    async fn call(
        &self,
        endpoint: &ManagedFeedbackEndpoint,
        session: &str,
        name: &str,
        arguments: Value,
    ) -> anyhow::Result<Value> {
        let response = self.post(endpoint, Some(session), json!({
            "jsonrpc":"2.0", "id":2, "method":"tools/call", "params":{"name":name,"arguments":arguments}
        })).send().await?.error_for_status()?;
        Ok(rpc_body(response).await?["result"].clone())
    }
}

async fn rpc_body(response: reqwest::Response) -> anyhow::Result<Value> {
    let body = response.text().await?;
    if let Ok(value) = serde_json::from_str(&body) {
        return Ok(value);
    }
    body.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .find_map(|payload| serde_json::from_str(payload.trim()).ok())
        .context("JSON-RPC SSE response")
}

fn request_input(request_id: &str) -> Value {
    json!({"request_id":request_id,"what_happened":"Please review this fixture", "actions":[{"id":"review","instruction":"Review the fixture"}]})
}

#[tokio::test]
async fn managed_transport_survives_long_user_idle_until_its_binding_is_revoked()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let a = fixture.provider.bind(&fixture.sessions[0]).await?;
    let b = fixture.provider.bind(&fixture.sessions[1]).await?;
    let sa = fixture.initialize(&a).await?;
    let sb = fixture.initialize(&b).await?;

    // Prevent the paused runtime from automatically skipping further deadlines
    // while advancing exactly sixteen idle minutes. No keepalive request is sent.
    let runnable = tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    });
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(16 * 60)).await;
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    tokio::time::resume();
    runnable.abort();

    let request_id = uuid::Uuid::now_v7().to_string();
    let result = fixture
        .call(&a, &sa, "request_feedback", request_input(&request_id))
        .await?;
    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(
        fixture
            .store
            .get_request(&request_id)
            .await?
            .managed_session_id
            .as_deref(),
        Some("managed-a")
    );
    for name in ["get_feedback", "recover_feedback"] {
        let result = fixture
            .call(&a, &sa, name, json!({"request_id":request_id}))
            .await?;
        assert_ne!(result["isError"], true, "{result}");
    }
    let list = json!({"jsonrpc":"2.0","id":3,"method":"tools/list"});
    assert_eq!(
        fixture
            .post(&b, Some(&sa), list.clone())
            .send()
            .await?
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    fixture.provider.revoke("managed-a").await?;
    assert_eq!(
        fixture
            .post(&a, Some(&sa), list.clone())
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert!(
        fixture
            .post(&b, Some(&sb), list)
            .send()
            .await?
            .status()
            .is_success()
    );
    fixture.server.shutdown().await?;
    fixture.store.close().await;
    Ok(())
}

#[tokio::test]
async fn managed_tools_fix_identity_and_reject_cross_scope_reads_and_external_spoofing()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let a = fixture.provider.bind(&fixture.sessions[0]).await?;
    let b = fixture.provider.bind(&fixture.sessions[1]).await?;
    let sa = fixture.initialize(&a).await?;
    let sb = fixture.initialize(&b).await?;
    let listed = fixture
        .post(
            &a,
            Some(&sa),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}),
        )
        .send()
        .await?
        .error_for_status()?;
    let tools = rpc_body(listed).await?["result"]["tools"]
        .as_array()
        .context("tools")?
        .clone();
    let mut names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["get_feedback", "recover_feedback", "request_feedback"]
    );
    let properties = &tools
        .iter()
        .find(|tool| tool["name"] == "request_feedback")
        .unwrap()["inputSchema"]["properties"];
    for key in ["host_id", "host_session_id", "managed_session_id"] {
        assert!(properties.get(key).is_none());
    }
    let request_id = uuid::Uuid::now_v7().to_string();
    let mut input = request_input(&request_id);
    input["host_id"] = json!("attacker");
    input["host_session_id"] = json!(fixture.sessions[1].host_session_id);
    input["managed_session_id"] = json!(fixture.sessions[1].session_id);
    let result = fixture.call(&a, &sa, "request_feedback", input).await?;
    assert_ne!(result["isError"], true, "{result}");
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("End this Agent turn now")
    );
    assert!(result["structuredContent"].get("poll_after_ms").is_none());
    assert!(result["structuredContent"].get("execution_mode").is_none());
    let stored = fixture.store.get_request(&request_id).await?;
    assert_eq!(stored.managed_session_id.as_deref(), Some("managed-a"));
    assert_eq!(stored.host_id, fixture.sessions[0].host_id);
    assert_eq!(stored.host_session_id, fixture.sessions[0].host_session_id);
    for name in ["get_feedback", "recover_feedback"] {
        let denied = fixture
            .call(&b, &sb, name, json!({"request_id":request_id}))
            .await?;
        assert_eq!(denied["isError"], true);
        assert_eq!(denied["structuredContent"]["code"], "REQUEST_NOT_FOUND");
    }
    let own = fixture
        .call(
            &a,
            &sa,
            "recover_feedback",
            json!({"host_id":"attacker","host_session_id":"managed-b"}),
        )
        .await?;
    assert_ne!(own["isError"], true);
    assert_eq!(own["structuredContent"]["request_id"], request_id);
    let mut spoof = request_input(&uuid::Uuid::now_v7().to_string());
    spoof["host_id"] = json!(fixture.sessions[0].host_id);
    spoof["host_session_id"] = json!(fixture.sessions[0].host_session_id);
    spoof["managed_session_id"] = json!(fixture.sessions[0].session_id);
    let response = fixture
        .client
        .post(format!(
            "http://{}/api/feedback/request",
            fixture.server.address()
        ))
        .bearer_auth(GLOBAL_TOKEN)
        .json(&spoof)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    fixture.server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn transport_sessions_cannot_cross_bindings_and_revoked_tokens_cannot_reconnect()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let a = fixture.provider.bind(&fixture.sessions[0]).await?;
    let b = fixture.provider.bind(&fixture.sessions[1]).await?;
    let sa = fixture.initialize(&a).await?;
    let sb = fixture.initialize(&b).await?;
    let list = json!({"jsonrpc":"2.0","id":3,"method":"tools/list"});
    let cross = fixture.post(&b, Some(&sa), list.clone()).send().await?;
    assert_eq!(cross.status(), reqwest::StatusCode::NOT_FOUND);
    fixture.provider.revoke("managed-a").await?;
    fixture.provider.revoke("managed-a").await?;
    for session in [None, Some(sa.as_str())] {
        assert_eq!(
            fixture
                .post(&a, session, list.clone())
                .send()
                .await?
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
    }
    assert!(
        fixture
            .post(&b, Some(&sb), list.clone())
            .send()
            .await?
            .status()
            .is_success()
    );
    let replacement = fixture.provider.bind(&fixture.sessions[0]).await?;
    assert!(replacement.bearer_token != a.bearer_token);
    assert_eq!(
        fixture
            .post(&replacement, Some(&sa), list.clone())
            .send()
            .await?
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    let current = fixture.initialize(&replacement).await?;
    // Rebinding a live instance also revokes its old token and transport session.
    let newest = fixture.provider.bind(&fixture.sessions[0]).await?;
    assert_eq!(
        fixture
            .post(&replacement, Some(&current), list.clone())
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    fixture.initialize(&newest).await?;
    fixture.server.shutdown().await?;
    assert!(fixture.provider.bind(&fixture.sessions[0]).await.is_err());
    Ok(())
}

#[tokio::test]
async fn scoped_route_enforces_host_origin_and_does_not_accept_global_credentials()
-> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let a = fixture.provider.bind(&fixture.sessions[0]).await?;
    for (header, value) in [
        ("Host", "attacker.example"),
        ("Origin", "https://attacker.example"),
    ] {
        assert_eq!(
            fixture
                .post(&a, None, json!({}))
                .header(header, value)
                .send()
                .await?
                .status(),
            reqwest::StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        fixture
            .client
            .post(&a.url)
            .bearer_auth(GLOBAL_TOKEN)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture.client.post(&a.url).send().await?.status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .client
            .post(fixture.server.endpoint())
            .bearer_auth(&a.bearer_token)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    fixture.initialize(&a).await?;
    fixture.server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn provider_has_one_listener_owner_and_can_restart_after_shutdown() -> anyhow::Result<()> {
    let fixture = Fixture::new().await?;
    let config = || {
        Ok::<_, anyhow::Error>(ServerConfig::new(AccessToken::parse(GLOBAL_TOKEN)?).with_port(0))
    };
    let duplicate = start_server_with_managed(
        config()?,
        fixture.store.clone().into_application(),
        fixture.provider.clone(),
    )
    .await;
    assert!(matches!(
        duplicate,
        Err(rambledesk_local_server::ServerError::ManagedFeedbackAlreadyBound)
    ));
    let old = fixture.provider.bind(&fixture.sessions[0]).await?;
    fixture.initialize(&old).await?;
    fixture.server.shutdown().await?;
    let server = start_server_with_managed(
        config()?,
        fixture.store.clone().into_application(),
        fixture.provider.clone(),
    )
    .await?;
    let new = fixture.provider.bind(&fixture.sessions[0]).await?;
    assert_eq!(new.url, format!("http://{}/mcp-managed", server.address()));
    assert!(old.bearer_token != new.bearer_token);
    assert_eq!(
        fixture
            .client
            .post(&new.url)
            .bearer_auth(&old.bearer_token)
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    server.shutdown().await?;
    Ok(())
}
