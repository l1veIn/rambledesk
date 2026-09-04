//! Persistent session identity and launch configuration contracts.
//!
//! ACP connections, process handles and live execution state are deliberately
//! absent: they are owned by the running application, not recovered from SQLite.

mod application;
mod driver;
mod model;
mod repository;
mod runtime;

pub use application::{SessionApplication, SessionError};
pub use driver::*;
pub use model::*;
pub use repository::{SessionRepository, SessionRepositoryError};
pub use runtime::*;
