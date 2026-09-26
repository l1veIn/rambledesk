use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ApplicationCommandFacade;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BrowseProjectDirectoriesInput {
    /// Null starts at the home directory of the device running RambleDesk.
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
pub struct ProjectDirectoryEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ProjectDirectoryListing {
    pub path: String,
    pub parent_path: Option<String>,
    pub home_path: Option<String>,
    pub roots: Vec<ProjectDirectoryEntry>,
    pub directories: Vec<ProjectDirectoryEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectDirectoryErrorCode {
    InvalidDirectoryPath,
    DirectoryNotFound,
    DirectoryAccessDenied,
    DirectoryUnavailable,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS, thiserror::Error)]
#[error("{message}")]
pub struct ProjectDirectoryError {
    pub code: ProjectDirectoryErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl ProjectDirectoryError {
    pub fn new(code: ProjectDirectoryErrorCode) -> Self {
        let message = match code {
            ProjectDirectoryErrorCode::InvalidDirectoryPath => "Choose an absolute directory path.",
            ProjectDirectoryErrorCode::DirectoryNotFound => "This directory no longer exists.",
            ProjectDirectoryErrorCode::DirectoryAccessDenied => {
                "This directory cannot be accessed."
            }
            ProjectDirectoryErrorCode::DirectoryUnavailable => {
                "This directory is unavailable. Try again."
            }
        };
        Self {
            code,
            message: message.into(),
            retryable: code == ProjectDirectoryErrorCode::DirectoryUnavailable,
        }
    }
}

/// Browses the host filesystem with the same OS identity used to start sessions.
/// Exposed only through the authenticated application surface, never Agent MCP.
#[async_trait]
pub trait ProjectDirectoryBrowser: Send + Sync {
    async fn browse(
        &self,
        input: BrowseProjectDirectoriesInput,
    ) -> Result<ProjectDirectoryListing, ProjectDirectoryError>;
}

impl ApplicationCommandFacade {
    pub fn with_project_directory_browser(
        mut self,
        browser: Arc<dyn ProjectDirectoryBrowser>,
    ) -> Self {
        self.project_directories = Some(browser);
        self
    }

    pub async fn browse_project_directories(
        &self,
        input: BrowseProjectDirectoriesInput,
    ) -> Result<ProjectDirectoryListing, ProjectDirectoryError> {
        self.project_directories
            .as_ref()
            .ok_or_else(|| {
                ProjectDirectoryError::new(ProjectDirectoryErrorCode::DirectoryUnavailable)
            })?
            .browse(input)
            .await
    }
}
