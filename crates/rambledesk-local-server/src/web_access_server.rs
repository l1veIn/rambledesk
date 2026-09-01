use std::{
    collections::VecDeque,
    net::Ipv4Addr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    body::Body,
    extract::{Extension, Request, State},
    http::{HeaderMap, HeaderValue, Method, Response, StatusCode, header},
    middleware::{self, Next},
    response::IntoResponse,
    routing::post,
};
use percent_encoding::percent_decode_str;
use rambledesk_core::{ApplicationChangeHub, ApplicationCommandFacade};
use thiserror::Error;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    WebAccessRouteConfig, WebSessionManager, web_access_router,
    web_security::{bearer_credential, has_exact_host, has_exact_host_and_origin, header_text},
};

pub const DEFAULT_WEB_ACCESS_PORT: u16 = 37_643;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaAsset {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub content_security_policy: Option<String>,
    pub cache_policy: SpaAssetCachePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaAssetCachePolicy {
    NoStore,
    NoCache,
    Immutable,
}

pub trait SpaAssetSource: Send + Sync {
    fn load(&self, path: &str) -> Option<SpaAsset>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebAccessServerConfig {
    pub port: u16,
    pub max_event_connections: usize,
    pub max_http_requests: usize,
    pub max_bootstrap_attempts_per_minute: usize,
}

impl Default for WebAccessServerConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_WEB_ACCESS_PORT,
            max_event_connections: 8,
            max_http_requests: 16,
            max_bootstrap_attempts_per_minute: 8,
        }
    }
}

pub struct WebAccessServerHandle {
    address: std::net::SocketAddr,
    origin: String,
    cancellation: CancellationToken,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl WebAccessServerHandle {
    pub fn address(&self) -> std::net::SocketAddr {
        self.address
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub async fn shutdown(self) -> Result<(), WebAccessServerError> {
        self.cancellation.cancel();
        self.task.await??;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum WebAccessServerError {
    #[error("failed to bind Web Access loopback listener: {0}")]
    Bind(#[source] std::io::Error),
    #[error("Web Access server failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Web Access server task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("invalid Web Access route configuration: {0}")]
    RouteConfig(String),
}

#[derive(Clone)]
struct SpaState {
    allowed_host: String,
    content_security_policy: String,
    assets: Arc<dyn SpaAssetSource>,
}

#[derive(Clone)]
struct BootstrapState {
    allowed_host: String,
    allowed_origin: String,
    sessions: Arc<WebSessionManager>,
    attempts: Arc<BootstrapRateLimiter>,
}

struct BootstrapRateLimiter {
    attempts: Mutex<VecDeque<Instant>>,
    max_attempts: usize,
    window: Duration,
}

impl BootstrapRateLimiter {
    fn per_minute(max_attempts: usize) -> Self {
        assert!(max_attempts > 0);
        Self {
            attempts: Mutex::new(VecDeque::new()),
            max_attempts,
            window: Duration::from_secs(60),
        }
    }

    fn admit(&self, now: Instant) -> bool {
        let mut attempts = self
            .attempts
            .lock()
            .expect("bootstrap rate limiter poisoned");
        while attempts
            .front()
            .is_some_and(|attempt| now.saturating_duration_since(*attempt) >= self.window)
        {
            attempts.pop_front();
        }
        if attempts.len() >= self.max_attempts {
            return false;
        }
        attempts.push_back(now);
        true
    }
}

#[derive(serde::Serialize)]
struct BootstrapSession {
    session_token: String,
}

pub async fn start_web_access_server(
    config: WebAccessServerConfig,
    commands: Arc<ApplicationCommandFacade>,
    changes: Arc<ApplicationChangeHub>,
    sessions: Arc<WebSessionManager>,
    assets: Arc<dyn SpaAssetSource>,
) -> Result<WebAccessServerHandle, WebAccessServerError> {
    if config.max_bootstrap_attempts_per_minute == 0 {
        return Err(WebAccessServerError::RouteConfig(
            "Web Access bootstrap rate limit must be non-zero".to_owned(),
        ));
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))
        .await
        .map_err(WebAccessServerError::Bind)?;
    let address = listener.local_addr().map_err(WebAccessServerError::Bind)?;
    let authority = address.to_string();
    let origin = format!("http://{authority}");
    let route_config = WebAccessRouteConfig::new(
        authority.clone(),
        origin.clone(),
        config.max_event_connections,
        config.max_http_requests,
    )
    .map_err(WebAccessServerError::RouteConfig)?;
    let spa_state = SpaState {
        allowed_host: authority.clone(),
        content_security_policy: format!(
            "default-src 'self'; img-src 'self' blob: data:; style-src 'self' 'unsafe-inline'; connect-src 'self' ws://{address}; frame-ancestors 'none'"
        ),
        assets,
    };
    let bootstrap_state = BootstrapState {
        allowed_host: authority.clone(),
        allowed_origin: origin.clone(),
        sessions: sessions.clone(),
        attempts: Arc::new(BootstrapRateLimiter::per_minute(
            config.max_bootstrap_attempts_per_minute,
        )),
    };
    let cancellation = CancellationToken::new();
    let api_router = web_access_router(
        commands,
        changes,
        sessions,
        route_config,
        cancellation.clone(),
    )
    .merge(
        Router::new()
            .route("/auth/session", post(bootstrap_session))
            .with_state(bootstrap_state),
    );
    let router = Router::new()
        .nest("/api", api_router)
        .fallback(serve_spa)
        .layer(Extension(Arc::new(spa_state.clone())))
        .layer(middleware::from_fn_with_state(spa_state, require_spa_host));
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { task_cancellation.cancelled_owned().await })
            .await
    });
    tracing::info!(%address, "RambleDesk Web Access server listening on loopback");
    Ok(WebAccessServerHandle {
        address,
        origin,
        cancellation,
        task,
    })
}

async fn bootstrap_session(
    State(state): State<BootstrapState>,
    headers: HeaderMap,
) -> Response<Body> {
    if !has_exact_host_and_origin(&headers, &state.allowed_host, &state.allowed_origin) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !state.attempts.admit(Instant::now()) {
        let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
        return response;
    }
    let durable_token = bearer_credential(headers.get(header::AUTHORIZATION));
    let Some(session_token) = durable_token.and_then(|token| state.sessions.issue_session(token))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let mut response = Json(BootstrapSession { session_token }).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

async fn require_spa_host(
    State(state): State<SpaState>,
    request: Request,
    next: Next,
) -> Response<Body> {
    if !has_exact_host(request.headers(), &state.allowed_host) {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

async fn serve_spa(Extension(state): Extension<Arc<SpaState>>, request: Request) -> Response<Body> {
    if request.uri().path() == "/api" || request.uri().path().starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let Some(path) = normalized_asset_path(request.uri().path()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let requested_asset = (!path.is_empty()).then_some(path.as_str());
    let (asset_path, asset, history_fallback) =
        match requested_asset.and_then(|path| state.assets.load(path).map(|asset| (path, asset))) {
            Some((path, asset)) => (path.to_owned(), asset, false),
            None if requested_asset
                .is_none_or(|path| !path.starts_with("assets/") && !has_file_extension(path))
                && accepts_html(&request) =>
            {
                let Some(index) = state.assets.load("index.html") else {
                    return StatusCode::SERVICE_UNAVAILABLE.into_response();
                };
                ("index.html".to_owned(), index, true)
            }
            None => return StatusCode::NOT_FOUND.into_response(),
        };
    let body = if request.method() == Method::HEAD {
        Body::empty()
    } else {
        Body::from(asset.bytes)
    };
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        if history_fallback || asset_path == "index.html" {
            HeaderValue::from_static("no-store")
        } else {
            match asset.cache_policy {
                SpaAssetCachePolicy::NoStore => HeaderValue::from_static("no-store"),
                SpaAssetCachePolicy::NoCache => HeaderValue::from_static("no-cache"),
                SpaAssetCachePolicy::Immutable => {
                    HeaderValue::from_static("public, max-age=31536000, immutable")
                }
            }
        },
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    let policy = asset.content_security_policy.or_else(|| {
        asset
            .mime_type
            .starts_with("text/html")
            .then(|| state.content_security_policy.clone())
    });
    if let Some(policy) = policy
        && let Ok(value) = HeaderValue::from_str(&policy)
    {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, value);
    }
    response
}

fn normalized_asset_path(raw_path: &str) -> Option<String> {
    let lowercase = raw_path.to_ascii_lowercase();
    if lowercase.contains("%2f") || lowercase.contains("%5c") {
        return None;
    }
    let decoded = percent_decode_str(raw_path).decode_utf8().ok()?;
    if decoded.contains(['\\', '\0']) || decoded.contains("//") {
        return None;
    }
    let path = decoded.strip_prefix('/')?;
    if path.split('/').any(|segment| segment.starts_with('.')) {
        return None;
    }
    Some(path.to_owned())
}

fn accepts_html(request: &Request) -> bool {
    request
        .headers()
        .get(header::ACCEPT)
        .and_then(header_text)
        .is_some_and(|accept| {
            accept
                .split(',')
                .map(|entry| entry.split(';').next().unwrap_or_default().trim())
                .any(|mime| matches!(mime, "text/html" | "application/xhtml+xml"))
        })
}

fn has_file_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_paths_reject_traversal_and_malformed_segments() {
        assert_eq!(
            normalized_asset_path("/assets/app-12345678.js"),
            Some("assets/app-12345678.js".into())
        );
        for path in [
            "../secret",
            "/../secret",
            "/%2e%2e/secret",
            "/a%2fb",
            "/a//b",
            "/a%5cb",
            "/.vite/manifest.json",
        ] {
            assert_eq!(normalized_asset_path(path), None, "{path}");
        }
    }
}
