//! SQLite persistence and Feedback Package publication boundary.

mod package;
mod platform;
mod sqlite;

/// ACP-first storage Adapters. This Module owns an independent schema and does
/// not connect through or migrate the frozen v2 store.
pub mod v3;

pub use sqlite::{
    SqliteFeedbackStore, StorageOpenError, default_app_data_root, default_database_path,
    default_library_path,
};
