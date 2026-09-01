use std::{collections::HashMap, sync::Arc};

use rambledesk_core::{
    ApplicationChangeHub, ApplicationCommandFacade, WorkbenchTerminalOperations,
};
use rambledesk_local_server::{
    SpaAsset, SpaAssetSource, WebAccessServerConfig, WebSessionAuthenticator,
    start_web_access_server,
};
use rambledesk_storage::SqliteFeedbackStore;

const SESSION_TOKEN: &str = "test-session-token";

struct TestAuthenticator;

impl WebSessionAuthenticator for TestAuthenticator {
    fn authenticate(&self, session_token: &str) -> bool {
        session_token == SESSION_TOKEN
    }
}

struct TestAssets {
    assets: HashMap<String, SpaAsset>,
}

impl SpaAssetSource for TestAssets {
    fn load(&self, path: &str) -> Option<SpaAsset> {
        self.assets.get(path).cloned()
    }
}

fn asset(bytes: &str, mime_type: &str) -> SpaAsset {
    SpaAsset {
        bytes: bytes.as_bytes().to_vec(),
        mime_type: mime_type.into(),
        content_security_policy: None,
    }
}

#[tokio::test]
async fn independent_server_serves_spa_history_and_fingerprinted_assets() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = SqliteFeedbackStore::connect(&directory.path().join("state.sqlite3")).await?;
    let changes = Arc::new(ApplicationChangeHub::with_runtime_generation("runtime-a"));
    let application = store
        .into_application()
        .with_change_observer(changes.clone());
    let commands = Arc::new(ApplicationCommandFacade::new(
        application.clone(),
        WorkbenchTerminalOperations::without_observer(application),
        vec![],
    ));
    let assets = Arc::new(TestAssets {
        assets: HashMap::from([
            (
                "index.html".into(),
                asset("<main>Workbench</main>", "text/html; charset=utf-8"),
            ),
            (
                "assets/app-1234abcd.js".into(),
                asset("export const ready = true", "text/javascript"),
            ),
        ]),
    });
    let server = start_web_access_server(
        WebAccessServerConfig {
            port: 0,
            max_event_connections: 2,
        },
        commands,
        changes,
        Arc::new(TestAuthenticator),
        assets,
    )
    .await?;
    let client = reqwest::Client::new();

    for path in ["/", "/sessions/codex/example"] {
        let response = client
            .get(format!("{}{path}", server.origin()))
            .header(reqwest::header::ACCEPT, "text/html")
            .send()
            .await?;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response.headers()[reqwest::header::CACHE_CONTROL],
            "no-store"
        );
        assert_eq!(response.text().await?, "<main>Workbench</main>");
    }

    let asset = client
        .get(format!("{}/assets/app-1234abcd.js", server.origin()))
        .send()
        .await?;
    assert_eq!(asset.status(), reqwest::StatusCode::OK);
    assert_eq!(
        asset.headers()[reqwest::header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(asset.headers()["x-content-type-options"], "nosniff");
    assert_eq!(asset.headers()["x-frame-options"], "DENY");

    for path in ["/assets/missing", "/missing.js", "/a%2Fb"] {
        let response = client
            .get(format!("{}{path}", server.origin()))
            .header(reqwest::header::ACCEPT, "text/html")
            .send()
            .await?;
        assert_ne!(response.status(), reqwest::StatusCode::OK, "{path}");
        assert_ne!(response.text().await?, "<main>Workbench</main>", "{path}");
    }

    let non_navigation = client
        .get(format!("{}/sessions/codex/example", server.origin()))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await?;
    assert_eq!(non_navigation.status(), reqwest::StatusCode::NOT_FOUND);

    let api_miss = client
        .post(format!("{}/api/not-an-operation", server.origin()))
        .header(reqwest::header::ORIGIN, server.origin())
        .bearer_auth(SESSION_TOKEN)
        .send()
        .await?;
    assert_eq!(api_miss.status(), reqwest::StatusCode::NOT_FOUND);
    assert_ne!(api_miss.text().await?, "<main>Workbench</main>");

    let wrong_host = client
        .get(server.origin())
        .header(reqwest::header::HOST, "attacker.example")
        .send()
        .await?;
    assert_eq!(wrong_host.status(), reqwest::StatusCode::FORBIDDEN);
    server.shutdown().await?;
    Ok(())
}
