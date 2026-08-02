//! SQLite persistence and Feedback Package publication boundary.

mod package;
mod platform;
mod sqlite;

pub use sqlite::{
    SqliteFeedbackStore, StorageOpenError, default_app_data_root, default_database_path,
    default_library_path,
};
