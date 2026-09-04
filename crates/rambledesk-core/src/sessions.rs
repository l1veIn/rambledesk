//! Persistent session identity and launch configuration contracts.
//!
//! ACP connections, process handles and live execution state are deliberately
//! absent: they are owned by the running application, not recovered from SQLite.

mod activity;
mod application;
mod continuation;
mod delivery;
mod deletion;
mod driver;
mod feedback_binding;
mod model;
mod permissions;
mod prompts;
mod repository;
mod runtime;

pub use activity::*;
pub use application::{SessionApplication, SessionError};
pub use continuation::ResolveFeedbackDeliveryInput;
pub use delivery::*;
pub use deletion::*;
pub use driver::*;
pub use feedback_binding::*;
pub use model::*;
pub use permissions::*;
pub use prompts::SendManagedPromptInput;
pub use repository::{SessionRepository, SessionRepositoryError};
pub use runtime::*;
