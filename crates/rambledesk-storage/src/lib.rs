//! SQLite persistence and feedback package adapter boundary.

mod package;
mod platform;
mod sqlite;

pub use sqlite::{SqliteFeedbackStore, StorageOpenError, default_database_path};
