use super::*;
use rambledesk_core::{
    AgentInstallJobInput, AgentManagementError, CatalogAgentInput, InstallAgentInput, ResolveCatalogAgentInput,
};

pub(super) fn routes() -> Router<ApplicationApiState> {
    Router::new()
        .route("/application/resolveCatalogAgent", post(resolve_catalog_agent))
        .route(
            "/application/listAvailableAgents",
            post(list_available_agents),
        )
        .route(
            "/application/inspectAgentInstallation",
            post(inspect_agent_installation),
        )
        .route(
            "/application/listAgentInstallJobs",
            post(list_agent_install_jobs),
        )
        .route("/application/installAgent", post(install_agent))
        .route(
            "/application/cancelAgentInstall",
            post(cancel_agent_install),
        )
}

fn result<T: Serialize>(result: Result<T, AgentManagementError>) -> Response<Body> {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => {
            let status = if error.code
                == rambledesk_core::ManagedCommandErrorCode::ManagedRuntimeUnavailable
            {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(error)).into_response()
        }
    }
}

async fn list_available_agents(State(state): State<ApplicationApiState>) -> Response<Body> {
    stable_result(&state, || async { state.commands.list_available_agents() }).await
}
async fn resolve_catalog_agent(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ResolveCatalogAgentInput>,
) -> Response<Body> {
    result(state.commands.resolve_catalog_agent(input).await)
}
async fn list_agent_install_jobs(State(state): State<ApplicationApiState>) -> Response<Body> {
    stable_result(&state, || async {
        state.commands.list_agent_install_jobs()
    })
    .await
}
async fn stable_result<T, Q, F>(state: &ApplicationApiState, query: Q) -> Response<Body>
where
    T: Serialize,
    Q: FnMut() -> F,
    F: std::future::Future<Output = Result<T, AgentManagementError>>,
{
    match state.changes.capture_snapshot(query).await {
        Ok(snapshot) => with_snapshot_metadata(result(Ok(snapshot.value)), &snapshot.metadata),
        Err(ApplicationSnapshotError::Query(error)) => result::<()>(Err(error)),
        Err(ApplicationSnapshotError::Unstable) => {
            unstable_snapshot_response(&state.changes.metadata())
        }
    }
}
async fn inspect_agent_installation(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<CatalogAgentInput>,
) -> Response<Body> {
    result(state.commands.inspect_agent_installation(input).await)
}
async fn install_agent(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<InstallAgentInput>,
) -> Response<Body> {
    result(state.commands.install_agent(input))
}
async fn cancel_agent_install(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<AgentInstallJobInput>,
) -> Response<Body> {
    match state.commands.cancel_agent_install(input).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => result::<()>(Err(error)),
    }
}
