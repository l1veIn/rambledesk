use rambledesk_core::{BrowseProjectDirectoriesInput, ProjectDirectoryErrorCode};

use super::*;

pub(super) fn routes() -> Router<ApplicationApiState> {
    Router::new().route(
        "/application/browseProjectDirectories",
        post(browse_project_directories),
    )
}

async fn browse_project_directories(
    State(state): State<ApplicationApiState>,
    ApplicationJson(input): ApplicationJson<BrowseProjectDirectoriesInput>,
) -> Response<Body> {
    let response = match state.commands.browse_project_directories(input).await {
        Ok(listing) => Json(listing).into_response(),
        Err(error) => {
            let status = match error.code {
                ProjectDirectoryErrorCode::InvalidDirectoryPath => StatusCode::BAD_REQUEST,
                ProjectDirectoryErrorCode::DirectoryNotFound => StatusCode::NOT_FOUND,
                // 403 is reserved for application authentication failures.
                ProjectDirectoryErrorCode::DirectoryAccessDenied => {
                    StatusCode::UNPROCESSABLE_ENTITY
                }
                ProjectDirectoryErrorCode::DirectoryUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            };
            (status, Json(error)).into_response()
        }
    };
    with_snapshot_metadata(response, &state.changes.metadata())
}
