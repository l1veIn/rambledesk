//! Persistent session identity and launch configuration contracts.
//!
//! ACP connections, process handles and live execution state are deliberately
//! absent: they are owned by the running application, not recovered from SQLite.

mod activity;
mod application;
mod driver;
mod model;
mod prompts;
mod repository;
mod runtime;

pub use activity::*;
pub use application::{SessionApplication, SessionError};
pub use driver::*;
pub use model::*;
pub use prompts::SendManagedPromptInput;
pub use repository::{SessionRepository, SessionRepositoryError};
pub use runtime::*;
