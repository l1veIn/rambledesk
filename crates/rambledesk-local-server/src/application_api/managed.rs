use rambledesk_core::{
    AgentConfigInput, CreateManagedSessionInput, ManagedCommandError, ManagedCommandErrorCode,
    ManagedSessionInput, PrepareManagedSessionInput, ResolveFeedbackDeliveryInput,
    RespondManagedPermissionInput, SaveAgentConfigInput, SendManagedPromptInput,
};

use super::*;

pub(super) fn routes() -> Router<ApplicationApiState> {
    Router::new()
        .route(
            "/application/listManagedSessionActivity",
            post(list_managed_session_activity),
        )
        .route(
            "/application/sendManagedPromptContent",
            post(send_managed_prompt_content).layer(DefaultBodyLimit::max(5 * 1024 * 1024)),
        )
        .route(
            "/application/setManagedSessionConfig",
            post(set_managed_session_config),
        )
        .route(
            "/application/deleteManagedSession",
            post(delete_managed_session),
        )
        .route(
            "/application/resolveFeedbackDelivery",
            post(resolve_feedback_delivery),
        )
        .route("/application/listAgentConfigs", post(list_agent_configs))
        .route("/application/saveAgentConfig", post(save_agent_config))
        .route("/application/deleteAgentConfig", post(delete_agent_config))
        .route("/application/checkAgentConfig", post(check_agent_config))
        .route(
            "/application/createManagedSession",
            post(create_managed_session),
        )
        .route(
            "/application/prepareManagedSession",
            post(prepare_managed_session),
        )
        .route(
            "/application/discardPreparedSession",
            post(discard_prepared_session),
        )
        .route("/application/getManagedSession", post(get_managed_session))
        .route(
            "/application/getManagedFeedbackStatus",
            post(get_managed_feedback_status),
        )
        .route(
            "/application/startManagedSession",
            post(start_managed_session),
        )
        .route(
            "/application/stopManagedSession",
            post(stop_managed_session),
        )
        .route("/application/sendManagedPrompt", post(send_managed_prompt))
        .route(
            "/application/cancelManagedPrompt",
            post(cancel_managed_prompt),
        )
        .route(
            "/application/respondManagedPermission",
            post(respond_managed_permission),
        )
}

fn error_response(error: ManagedCommandError) -> Response<Body> {
    use ManagedCommandErrorCode as Code;
    let status = match error.code {
        Code::InvalidArgument | Code::SessionNotManaged => StatusCode::BAD_REQUEST,
        Code::ManagedSessionNotFound | Code::AgentConfigNotFound => StatusCode::NOT_FOUND,
        Code::AgentConfigInUse
        | Code::AgentConfigDisabled
        | Code::ManagedSessionConflict
        | Code::ManagedSessionBusy
        | Code::ManagedSessionNotConnected
        | Code::ManagedSessionInterrupted => StatusCode::CONFLICT,
        Code::ManagedRuntimeUnavailable | Code::RuntimeShuttingDown => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        Code::StorageFailure | Code::AgentOperationFailed => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(error)).into_response()
}

fn managed_result<T: Serialize>(result: Result<T, ManagedCommandError>) -> Response<Body> {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}

async fn stable_managed_result<Value, Query, QueryFuture>(
    state: &ApplicationApiState,
    query: Query,
) -> Response<Body>
where
    Value: Serialize,
    Query: FnMut() -> QueryFuture,
    QueryFuture: std::future::Future<Output = Result<Value, ManagedCommandError>>,
{
    match state.changes.capture_snapshot(query).await {
        Ok(snapshot) => {
            with_snapshot_metadata(managed_result(Ok(snapshot.value)), &snapshot.metadata)
        }
        Err(ApplicationSnapshotError::Query(error)) => {
            with_snapshot_metadata(error_response(error), &state.changes.metadata())
        }
        Err(ApplicationSnapshotError::Unstable) => {
            unstable_snapshot_response(&state.changes.metadata())
        }
    }
}

async fn list_agent_configs(State(state): State<ApplicationApiState>) -> Response<Body> {
    stable_managed_result(&state, || state.commands.list_agent_configs()).await
}
async fn save_agent_config(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<SaveAgentConfigInput>,
) -> Response<Body> {
    managed_result(state.commands.save_agent_config(input).await)
}
async fn delete_agent_config(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<AgentConfigInput>,
) -> Response<Body> {
    match state.commands.delete_agent_config(input).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error_response(error),
    }
}
async fn check_agent_config(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<AgentConfigInput>,
) -> Response<Body> {
    managed_result(state.commands.check_agent_config(input).await)
}
async fn create_managed_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<CreateManagedSessionInput>,
) -> Response<Body> {
    managed_result(state.commands.create_managed_session(input).await)
}
async fn get_managed_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ManagedSessionInput>,
) -> Response<Body> {
    stable_managed_result(&state, || state.commands.get_managed_session(input.clone())).await
}

async fn get_managed_feedback_status(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ManagedSessionInput>,
) -> Response<Body> {
    stable_managed_result(&state, || {
        state.commands.get_managed_feedback_status(input.clone())
    })
    .await
}

async fn prepare_managed_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<PrepareManagedSessionInput>,
) -> Response<Body> {
    managed_result(state.commands.prepare_managed_session(input).await)
}

async fn discard_prepared_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ManagedSessionInput>,
) -> Response<Body> {
    match state.commands.discard_prepared_session(input).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error_response(error),
    }
}
async fn start_managed_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ManagedSessionInput>,
) -> Response<Body> {
    managed_result(state.commands.start_managed_session(input).await)
}
async fn stop_managed_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ManagedSessionInput>,
) -> Response<Body> {
    managed_result(state.commands.stop_managed_session(input).await)
}
async fn send_managed_prompt(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<SendManagedPromptInput>,
) -> Response<Body> {
    managed_result(state.commands.send_managed_prompt(input).await)
}
async fn cancel_managed_prompt(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ManagedSessionInput>,
) -> Response<Body> {
    managed_result(state.commands.cancel_managed_prompt(input).await)
}
async fn respond_managed_permission(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<RespondManagedPermissionInput>,
) -> Response<Body> {
    managed_result(state.commands.respond_managed_permission(input).await)
}

async fn resolve_feedback_delivery(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ResolveFeedbackDeliveryInput>,
) -> Response<Body> {
    managed_result(state.commands.resolve_feedback_delivery(input).await)
}

async fn delete_managed_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ManagedSessionInput>,
) -> Response<Body> {
    match state.commands.delete_managed_session(input).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error_response(error),
    }
}

async fn set_managed_session_config(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<rambledesk_core::SetManagedSessionConfigInput>,
) -> Response<Body> {
    managed_result(state.commands.set_managed_session_config(input).await)
}

async fn send_managed_prompt_content(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<rambledesk_core::SendManagedPromptContentInput>,
) -> Response<Body> {
    managed_result(state.commands.send_managed_prompt_content(input).await)
}

async fn list_managed_session_activity(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<rambledesk_core::ListManagedSessionActivityInput>,
) -> Response<Body> {
    stable_managed_result(&state, || {
        state.commands.list_managed_session_activity(input.clone())
    })
    .await
}
