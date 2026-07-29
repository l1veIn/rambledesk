//! SQLite persistence and feedback package adapter boundary.

mod package;
mod sqlite;

use rambledesk_core::HealthSnapshot;

pub use sqlite::{SqliteFeedbackStore, StorageOpenError, default_database_path};

pub fn health_snapshot() -> HealthSnapshot {
    HealthSnapshot::ready()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_ready_when_the_storage_adapter_is_initialized() {
        assert_eq!(
            health_snapshot().storage,
            rambledesk_core::StorageStatus::Ready
        );
    }
}
