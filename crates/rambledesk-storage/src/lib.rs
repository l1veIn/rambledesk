//! SQLite persistence and Feedback Package publication boundary.

mod package;
mod platform;
mod sqlite;

/// v3 storage Adapters. This Module owns an independent schema and does not
/// implicitly connect through or migrate the Adapter Runtime store.
pub mod v3;

pub use sqlite::{
    SqliteFeedbackStore, StorageOpenError, default_app_data_root, default_database_path,
    default_library_path,
};
