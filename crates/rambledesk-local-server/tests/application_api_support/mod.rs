use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{
    Router,
    body::Body,
    http::{Request, Response, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::IntoResponse,
};
use rambledesk_core::{
    ApplicationChangeHub, ApplicationCommandFacade, ApplicationHostProfileView,
    FeedbackApplication, WorkbenchTerminalOperations,
};
use rambledesk_local_server::{API_PATH, RUNTIME_GENERATION_HEADER, application_router};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

const TEST_AUTHORIZATION: &str =
    "Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub struct ApplicationServer {
    address: SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<std::io::Result<()>>,
}

impl ApplicationServer {
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        self.task.await??;
        Ok(())
    }
}

pub async fn start_application_server(
    application: FeedbackApplication,
    terminal_operations: WorkbenchTerminalOperations,
) -> anyhow::Result<ApplicationServer> {
    let router = Router::new()
        .nest(
            API_PATH,
            application_router(
                Arc::new(ApplicationCommandFacade::new(
                    application,
                    terminal_operations,
                    test_host_profiles(),
                )),
                Arc::new(ApplicationChangeHub::with_runtime_generation(
                    "test-runtime",
                )),
            ),
        )
        .layer(middleware::from_fn(require_test_bearer));
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let address = listener.local_addr()?;
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { task_cancellation.cancelled_owned().await })
            .await
    });
    Ok(ApplicationServer {
        address,
        cancellation,
        task,
    })
}

async fn require_test_bearer(mut request: Request<Body>, next: Next) -> Response<Body> {
    let authorized = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        == Some(TEST_AUTHORIZATION);
    if authorized {
        request.headers_mut().insert(
            RUNTIME_GENERATION_HEADER,
            "test-runtime".parse().expect("test runtime generation"),
        );
        next.run(request).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

fn test_host_profiles() -> Vec<ApplicationHostProfileView> {
    vec![ApplicationHostProfileView {
        id: "codex".into(),
        label: "Codex".into(),
        icon_svg: "<svg />".into(),
        default_adapter: "generic_mcp".into(),
        continuation_mode: "manual".into(),
    }]
}
