use super::*;
use rambledesk_core::{
    OpenTerminalSessionInput, ReadTerminalSessionInput, ResizeTerminalSessionInput,
    TerminalSessionInput, WriteTerminalSessionInput,
};

pub(super) fn routes() -> Router<ApplicationApiState> {
    Router::new()
        .route(
            "/application/openTerminalSession",
            post(open_terminal_session),
        )
        .route(
            "/application/readTerminalSession",
            post(read_terminal_session),
        )
        .route(
            "/application/writeTerminalSession",
            post(write_terminal_session),
        )
        .route(
            "/application/resizeTerminalSession",
            post(resize_terminal_session),
        )
        .route(
            "/application/stopTerminalSession",
            post(stop_terminal_session),
        )
}

async fn open_terminal_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<OpenTerminalSessionInput>,
) -> Response<Body> {
    with_snapshot_metadata(
        application_result(state.commands.open_terminal_session(input).await),
        &state.changes.metadata(),
    )
}
async fn read_terminal_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ReadTerminalSessionInput>,
) -> Response<Body> {
    with_snapshot_metadata(
        application_result(state.commands.read_terminal_session(input).await),
        &state.changes.metadata(),
    )
}
async fn write_terminal_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<WriteTerminalSessionInput>,
) -> Response<Body> {
    with_snapshot_metadata(
        application_result(state.commands.write_terminal_session(input).await),
        &state.changes.metadata(),
    )
}
async fn resize_terminal_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<ResizeTerminalSessionInput>,
) -> Response<Body> {
    with_snapshot_metadata(
        application_result(state.commands.resize_terminal_session(input).await),
        &state.changes.metadata(),
    )
}
async fn stop_terminal_session(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<TerminalSessionInput>,
) -> Response<Body> {
    with_snapshot_metadata(
        application_result(state.commands.stop_terminal_session(input).await),
        &state.changes.metadata(),
    )
}
