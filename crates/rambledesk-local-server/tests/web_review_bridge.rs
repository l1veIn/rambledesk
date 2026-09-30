use rambledesk_local_server::{AccessToken, ServerConfig, start_server};
use rambledesk_storage::SqliteFeedbackStore;

#[tokio::test]
async fn installed_server_exposes_bridge_bytes_without_exposing_authenticated_apis()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = SqliteFeedbackStore::connect(&directory.path().join("bridge.sqlite3")).await?;
    let server = start_server(
        ServerConfig::new(AccessToken::generate()).with_port(0),
        store.clone().into_application(),
    )
    .await?;
    let origin = format!("http://{}", server.address());
    let client = reqwest::Client::new();
    let bridge = client
        .get(format!("{origin}/web-review/bridge.js"))
        .header("Origin", "http://127.0.0.1:3000")
        .send()
        .await?;
    assert_eq!(bridge.status(), reqwest::StatusCode::OK);
    assert_eq!(
        bridge.headers()["content-type"],
        "text/javascript; charset=utf-8"
    );
    assert_eq!(
        bridge.text().await?,
        include_str!("../../../apps/desktop/public/rambledesk-web-review.js")
    );
    assert_eq!(
        client
            .get(format!("{origin}/api/health"))
            .send()
            .await?
            .status(),
        reqwest::StatusCode::UNAUTHORIZED,
    );
    assert_eq!(
        client
            .get(format!("{origin}/web-review/bridge.js"))
            .header("Host", "attacker.example")
            .send()
            .await?
            .status(),
        reqwest::StatusCode::FORBIDDEN,
    );
    server.shutdown().await?;
    store.close().await;
    Ok(())
}
