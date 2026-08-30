use serde::{Deserialize, Serialize};

pub(crate) const MIGRATION_REPORT_SCHEMA: &str = "rambledesk-v2-to-v3-migration-v1";
pub(crate) const VERIFY_REPORT_SCHEMA: &str = "rambledesk-v3-verify-v1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationCounts {
    pub sessions_created: u64,
    pub waiting_requests_migrated: u64,
    pub submitted_requests_migrated: u64,
    pub drafts_migrated: u64,
    pub artifacts_migrated: u64,
    pub records_dropped: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationLoss {
    pub legacy_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMapping {
    pub legacy_session_record_id: String,
    pub legacy_host_id: String,
    pub legacy_host_session_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationOutputs {
    pub database: String,
    pub database_sha256: String,
    pub artifact_library: String,
    pub backup_database: String,
    pub backup_database_sha256: String,
    pub backup_objects: String,
    pub backup_index: String,
    pub backup_objects_count: u64,
    pub json_report: String,
    pub markdown_report: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    pub report_schema: String,
    pub mode: String,
    pub source_schema: String,
    pub target_schema: String,
    pub started_at: String,
    pub finished_at: String,
    pub source_database_sha256: String,
    pub counts: MigrationCounts,
    pub session_mappings: Vec<SessionMapping>,
    pub losses: Vec<MigrationLoss>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<MigrationOutputs>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyCounts {
    pub sessions: u64,
    pub waiting_requests: u64,
    pub submitted_requests: u64,
    pub drafts: u64,
    pub packages: u64,
    pub delivered_deliveries: u64,
    pub artifact_objects: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyReport {
    pub report_schema: String,
    pub mode: String,
    pub target_schema: String,
    pub valid: bool,
    pub target_database_sha256: String,
    pub counts: VerifyCounts,
    pub checks: Vec<VerifyCheck>,
}
