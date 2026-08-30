//! ACP-first storage Adapters.

pub mod artifact;
mod sqlite;

pub use sqlite::{SqliteV3OpenError, SqliteV3Store, V3ConsistencyReport};
