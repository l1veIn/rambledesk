//! Public, credential-free bridge bytes. Business APIs keep their own authentication.
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

const BRIDGE: &str = include_str!("../../../apps/desktop/public/rambledesk-web-review.js");

pub(crate) fn router(allowed_host: String) -> Router {
    Router::new()
        .route("/web-review/bridge.js", get(serve_bridge))
        .with_state(allowed_host)
}

async fn serve_bridge(State(allowed_host): State<String>, headers: HeaderMap) -> Response<Body> {
    if !crate::web_security::has_exact_host(&headers, &allowed_host) {
        return StatusCode::FORBIDDEN.into_response();
    }
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        BRIDGE,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bridge_has_no_credentials_and_only_accepts_the_listener_host() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:37642".parse().unwrap());
        let response = serve_bridge(State("127.0.0.1:37642".into()), headers.clone()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        let body = axum::body::to_bytes(response.into_body(), 100_000)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), BRIDGE.as_bytes());
        assert!(BRIDGE.contains("rambledesk.web-review.v1"));
        headers.insert(header::HOST, "attacker.example:37642".parse().unwrap());
        assert_eq!(
            serve_bridge(State("127.0.0.1:37642".into()), headers)
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
}
