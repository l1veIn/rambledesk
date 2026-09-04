//! Persistent session identity and launch configuration contracts.
//!
//! ACP connections, process handles and live execution state are deliberately
//! absent: they are owned by the running application, not recovered from SQLite.

mod activity;
mod agent_catalog;
mod application;
mod continuation;
mod deletion;
mod delivery;
mod driver;
mod feedback_binding;
mod model;
mod permissions;
mod prompts;
mod recovery;
mod recovery_runtime;
mod repository;
mod runtime;

pub use activity::*;
pub use agent_catalog::*;
pub use application::{SessionApplication, SessionError};
pub use continuation::ResolveFeedbackDeliveryInput;
pub use deletion::*;
pub use delivery::*;
pub use driver::*;
pub use feedback_binding::*;
pub use model::*;
pub use permissions::*;
pub use prompts::SendManagedPromptInput;
pub use recovery::*;
pub use repository::{SessionRepository, SessionRepositoryError};
pub use runtime::*;
