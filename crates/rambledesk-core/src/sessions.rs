//! Persistent session identity and launch configuration contracts.
//!
//! ACP connections, process handles and live execution state are deliberately
//! absent: they are owned by the running application, not recovered from SQLite.

mod activity;
mod activity_history;
mod agent_catalog;
mod agent_check;
mod application;
mod configuration;
mod continuation;
mod deletion;
mod delivery;
mod driver;
mod failure;
mod feedback_binding;
mod feedback_status;
mod interactions;
mod model;
mod prompt_content;
mod prompts;
mod recovery;
mod recovery_runtime;
mod repository;
mod runtime;
mod workspace_info;

pub use activity::*;
pub use activity_history::*;
pub use agent_catalog::*;
pub use agent_check::*;
pub use application::{SessionApplication, SessionError};
pub use configuration::*;
pub use continuation::ResolveFeedbackDeliveryInput;
pub use deletion::*;
pub use delivery::*;
pub use driver::*;
pub use failure::*;
pub use feedback_binding::*;
pub use feedback_status::*;
pub use interactions::*;
pub use model::*;
pub use prompt_content::*;
pub use prompts::SendManagedPromptInput;
pub use recovery::*;
pub use repository::{SessionRepository, SessionRepositoryError};
pub use runtime::*;
pub use workspace_info::*;
