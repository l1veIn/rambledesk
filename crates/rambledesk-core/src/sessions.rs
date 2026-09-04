//! Persistent session identity and launch configuration contracts.
//!
//! ACP connections, process handles and live execution state are deliberately
//! absent: they are owned by the running application, not recovered from SQLite.

mod model;
mod repository;

pub use model::*;
pub use repository::{SessionRepository, SessionRepositoryError};
