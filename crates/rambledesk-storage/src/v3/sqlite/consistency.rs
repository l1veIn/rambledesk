use std::collections::BTreeMap;

use rambledesk_core::kernel::{PackageId, ports::FactStoreError};
use sqlx::Row;

use super::{SqliteV3Store, read::load_package, storage_error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V3ConsistencyReport {
    pub generation: Option<(u32, u32)>,
    pub table_counts: BTreeMap<String, u64>,
    pub violations: Vec<String>,
}

impl V3ConsistencyReport {
    pub fn is_consistent(&self) -> bool {
        self.generation == Some((3, 1)) && self.violations.is_empty()
    }
}

impl SqliteV3Store {
    /// Adapter-specific, read-only consistency report. This deliberately stays
    /// outside the Core FactStore Interface.
    pub async fn inspect_consistency(&self) -> Result<V3ConsistencyReport, FactStoreError> {
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let connection = transaction.as_mut();
        let marker: Option<(i64, i64)> = sqlx::query_as(
            "SELECT generation, revision FROM schema_generation_v3 WHERE singleton = 1",
        )
        .fetch_optional(&mut *connection)
        .await
        .map_err(storage_error)?;
        let generation = marker.and_then(|(generation, revision)| {
            Some((
                u32::try_from(generation).ok()?,
                u32::try_from(revision).ok()?,
            ))
        });
        let mut violations = Vec::new();
        if generation != Some((3, 1)) {
            violations.push("schema_generation_v3 marker is not (3, 1)".to_owned());
        }

        for row in sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut *connection)
            .await
            .map_err(storage_error)?
        {
            violations.push(format!(
                "foreign key violation: table={} rowid={:?} parent={} fk={}",
                row.get::<String, _>("table"),
                row.get::<Option<i64>, _>("rowid"),
                row.get::<String, _>("parent"),
                row.get::<i64, _>("fkid")
            ));
        }

        let tables = [
            "sessions_v3",
            "acp_session_links_v3",
            "artifact_objects_v3",
            "feedback_requests_v3",
            "ramble_submissions_v3",
            "ramble_drafts_v3",
            "packages_v3",
            "feedback_deliveries_v3",
            "agent_work_v3",
        ];
        let mut table_counts = BTreeMap::new();
        for table in tables {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&mut *connection)
                .await
                .map_err(storage_error)?;
            let count = u64::try_from(count).map_err(|_| FactStoreError::CorruptData)?;
            table_counts.insert(table.to_owned(), count);
        }

        let artifact_mismatches: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (
                SELECT storage_key, sha256, size_bytes FROM feedback_request_artifacts_v3
                UNION ALL SELECT storage_key, sha256, size_bytes FROM submission_artifacts_v3
                UNION ALL SELECT storage_key, sha256, size_bytes FROM draft_artifacts_v3
                UNION ALL SELECT storage_key, sha256, size_bytes FROM package_artifacts_v3
             ) entries
             LEFT JOIN artifact_objects_v3 objects USING (storage_key)
             WHERE objects.storage_key IS NULL
                OR objects.sha256 != entries.sha256
                OR objects.size_bytes != entries.size_bytes",
        )
        .fetch_one(&mut *connection)
        .await
        .map_err(storage_error)?;
        if artifact_mismatches != 0 {
            violations.push(format!(
                "{artifact_mismatches} artifact entries are dangling or mismatch their object"
            ));
        }

        let lifecycle_mismatches: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (
                SELECT r.request_id
                FROM feedback_requests_v3 r
                LEFT JOIN feedback_deliveries_v3 d ON d.request_id = r.request_id
                WHERE (r.resolution IS NULL AND d.delivery_id IS NOT NULL)
                   OR (r.resolution IS NOT NULL AND d.delivery_id IS NULL)
                   OR (d.delivery_id IS NOT NULL AND (
                        d.session_id != r.session_id
                        OR d.resolution != r.resolution
                        OR d.package_id IS NOT r.response_package_id
                   ))
                UNION ALL
                SELECT d.delivery_id
                FROM feedback_deliveries_v3 d
                LEFT JOIN agent_work_v3 w ON w.source_delivery_id = d.delivery_id
                WHERE (d.state = 'pending' AND (w.work_id IS NULL OR w.state = 'completed'))
                   OR (d.state = 'delivered' AND w.work_id IS NOT NULL AND w.state != 'completed')
             )",
        )
        .fetch_one(&mut *connection)
        .await
        .map_err(storage_error)?;
        if lifecycle_mismatches != 0 {
            violations.push(format!(
                "{lifecycle_mismatches} terminal request/delivery/work facts disagree"
            ));
        }

        let package_rows = sqlx::query("SELECT package_id FROM packages_v3 ORDER BY package_id")
            .fetch_all(&mut *connection)
            .await
            .map_err(storage_error)?;
        for row in package_rows {
            let package_id = PackageId::new(row.get::<String, _>("package_id"));
            match load_package(connection, &package_id).await {
                Ok(Some(_)) => {}
                Ok(None) => violations.push(format!("Package {package_id} disappeared")),
                Err(_) => violations.push(format!(
                    "Package {package_id} manifest does not match normalized facts"
                )),
            }
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(V3ConsistencyReport {
            generation,
            table_counts,
            violations,
        })
    }
}
