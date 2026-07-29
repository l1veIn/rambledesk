//! Storage adapter boundary. SQLite and package publishing arrive in M1.

use rambledesk_core::HealthSnapshot;

pub fn health_snapshot() -> HealthSnapshot {
    HealthSnapshot::m0()
}

#[cfg(test)]
mod tests {
    use rambledesk_core::StorageStatus;

    use super::*;

    #[test]
    fn m0_explicitly_reports_uninitialized_storage() {
        assert_eq!(health_snapshot().storage, StorageStatus::NotInitialized);
    }
}
