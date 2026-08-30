use std::{
    collections::BTreeSet,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use thiserror::Error;

use crate::{
    digest::{bytes_digest, deterministic_id},
    inspect::{InspectError, InspectReport, file_sha256, reject_active_wal},
    legacy_v2::{LegacyDataset, load_dataset},
    model::{
        MIGRATION_REPORT_SCHEMA, MigrationCounts, MigrationOutputs, MigrationReport,
        SessionMapping, VerifyReport,
    },
    target_v3::{build_artifacts_and_backup, verify_published_root, verify_root, write_database},
};

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error("the target root already exists; migration only publishes into a new root")]
    TargetExists,
    #[error("the target root must have an existing, real parent directory")]
    InvalidTargetParent,
    #[error("the target root must be a named child directory, not a filesystem root")]
    InvalidTargetRoot,
    #[error("the target database has an active WAL or shared-memory sidecar")]
    TargetActiveWal,
    #[error("failed to open the immutable v2 source")]
    SourceOpen(#[source] sqlx::Error),
    #[error("failed to write the v3 target database")]
    TargetDatabase(#[source] sqlx::Error),
    #[error("failed to install the v3 schema")]
    TargetMigration(#[source] sqlx::migrate::MigrateError),
    #[error("failed to write the atomic target root")]
    WriteTarget(#[source] std::io::Error),
    #[error("failed to serialize a migration artifact")]
    Serialize(#[source] serde_json::Error),
    #[error("the staged v3 target did not pass verification")]
    VerificationFailed,
    #[error("migration invariant failed: {0}")]
    Invariant(String),
    #[error("legacy request {legacy_id} cannot form valid v3 facts: {reason}")]
    InvalidLegacyRequest { legacy_id: String, reason: String },
}

struct MigrationPlan {
    source_db: PathBuf,
    inspected: InspectReport,
    dataset: LegacyDataset,
}

pub async fn dry_run(
    source_db: &Path,
    target_root: &Path,
) -> Result<MigrationReport, MigrationError> {
    let started_at = now();
    let target_root = validate_new_target_root(target_root).await?;
    let plan = build_plan(source_db).await?;
    let parent = target_root
        .parent()
        .ok_or(MigrationError::InvalidTargetParent)?;
    let temporary = tempfile::Builder::new()
        .prefix(".rambledesk-v3-dry-run-")
        .tempdir_in(parent)
        .map_err(MigrationError::WriteTarget)?;
    let dry_root = temporary.path();
    let backup = dry_root.join("backup");
    tokio::fs::create_dir(&backup)
        .await
        .map_err(MigrationError::WriteTarget)?;
    copy_new_file(&plan.source_db, &backup.join("source.sqlite3")).await?;
    let (artifacts, _) = build_artifacts_and_backup(
        dry_root,
        &plan.dataset,
        &plan.inspected.source_database_sha256,
    )
    .await?;
    write_database(
        &dry_root.join("rambledesk-v3.sqlite3"),
        &plan.dataset,
        &artifacts,
        &plan.inspected.source_database_sha256,
    )
    .await?;
    if !verify_root(dry_root).await?.valid {
        return Err(MigrationError::VerificationFailed);
    }
    reject_active_wal(&plan.source_db).await?;
    if file_sha256(&plan.source_db).await? != plan.inspected.source_database_sha256 {
        return Err(MigrationError::Invariant(
            "v2 source changed during dry-run".to_owned(),
        ));
    }
    Ok(build_report("dry_run", started_at, now(), &plan, None))
}

pub async fn execute(
    source_db: &Path,
    target_root: &Path,
) -> Result<MigrationReport, MigrationError> {
    let started_at = now();
    let target_root = validate_new_target_root(target_root).await?;
    let plan = build_plan(source_db).await?;
    let staging_root = staging_path(&target_root)?;
    tokio::fs::create_dir(&staging_root)
        .await
        .map_err(MigrationError::WriteTarget)?;
    secure_directory(&staging_root).await?;
    let result = execute_in_staging(&plan, &staging_root, started_at).await;
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            cleanup_staging(&staging_root).await;
            return Err(error);
        }
    };
    if let Err(error) = publish_no_replace(&staging_root, &target_root).await {
        cleanup_staging(&staging_root).await;
        return Err(error);
    }
    sync_directory(
        target_root
            .parent()
            .ok_or(MigrationError::InvalidTargetParent)?,
    )
    .await?;
    Ok(report)
}

pub async fn verify(target_root: &Path) -> Result<VerifyReport, MigrationError> {
    verify_published_root(target_root).await
}

async fn execute_in_staging(
    plan: &MigrationPlan,
    staging_root: &Path,
    started_at: String,
) -> Result<MigrationReport, MigrationError> {
    let backup = staging_root.join("backup");
    tokio::fs::create_dir_all(&backup)
        .await
        .map_err(MigrationError::WriteTarget)?;
    secure_directory(&backup).await?;
    copy_new_file(&plan.source_db, &backup.join("source.sqlite3")).await?;
    let backup_digest = file_sha256(&backup.join("source.sqlite3")).await?;
    if backup_digest != plan.inspected.source_database_sha256 {
        return Err(MigrationError::Invariant(
            "source backup digest does not match inspected database".to_owned(),
        ));
    }
    make_read_only(&backup.join("source.sqlite3")).await?;
    let (artifacts, backup_result) = build_artifacts_and_backup(
        staging_root,
        &plan.dataset,
        &plan.inspected.source_database_sha256,
    )
    .await?;
    let target_db = staging_root.join("rambledesk-v3.sqlite3");
    write_database(
        &target_db,
        &plan.dataset,
        &artifacts,
        &plan.inspected.source_database_sha256,
    )
    .await?;
    let staged_verification = verify_root(staging_root).await?;
    if !staged_verification.valid {
        return Err(MigrationError::VerificationFailed);
    }
    reject_active_wal(&plan.source_db).await?;
    if file_sha256(&plan.source_db).await? != plan.inspected.source_database_sha256 {
        return Err(MigrationError::Invariant(
            "v2 source changed during migration".to_owned(),
        ));
    }
    let outputs = MigrationOutputs {
        database: "rambledesk-v3.sqlite3".to_owned(),
        database_sha256: file_sha256(&target_db).await?,
        artifact_library: "library/artifacts".to_owned(),
        backup_database: "backup/source.sqlite3".to_owned(),
        backup_database_sha256: backup_digest,
        backup_objects: "backup/legacy-library/objects".to_owned(),
        backup_index: "backup/legacy-library/index.json".to_owned(),
        backup_objects_count: backup_result.object_count,
        json_report: "reports/migration-report.json".to_owned(),
        markdown_report: "reports/migration-report.md".to_owned(),
    };
    let report = build_report("execute", started_at, now(), plan, Some(outputs));
    write_reports(staging_root, &report).await?;
    make_tree_read_only(&backup).await?;
    sync_tree(staging_root).await?;
    Ok(report)
}

async fn build_plan(source_db: &Path) -> Result<MigrationPlan, MigrationError> {
    let source_db = tokio::fs::canonicalize(source_db)
        .await
        .map_err(|error| MigrationError::Inspect(InspectError::SourceRead(error)))?;
    let inspected = crate::inspect(&source_db).await?;
    let options = SqliteConnectOptions::new()
        .filename(&source_db)
        .create_if_missing(false)
        .read_only(true)
        .immutable(true)
        .foreign_keys(true);
    let pool = SqlitePool::connect_with(options)
        .await
        .map_err(MigrationError::SourceOpen)?;
    sqlx::query("PRAGMA query_only = ON")
        .execute(&pool)
        .await
        .map_err(MigrationError::SourceOpen)?;
    let dataset = load_dataset(&pool, &inspected, &source_db).await?;
    pool.close().await;
    reject_active_wal(&source_db).await?;
    if file_sha256(&source_db).await? != inspected.source_database_sha256 {
        return Err(MigrationError::Invariant(
            "v2 source changed while building the migration plan".to_owned(),
        ));
    }
    Ok(MigrationPlan {
        source_db,
        inspected,
        dataset,
    })
}

fn build_report(
    mode: &str,
    started_at: String,
    finished_at: String,
    plan: &MigrationPlan,
    outputs: Option<MigrationOutputs>,
) -> MigrationReport {
    let submitted = plan
        .dataset
        .requests
        .iter()
        .filter(|request| !request.waiting)
        .count() as u64;
    let waiting = plan.dataset.requests.len() as u64 - submitted;
    let drafts = plan
        .dataset
        .requests
        .iter()
        .filter(|request| request.waiting && request.draft.is_some())
        .count() as u64;
    let session_mappings = plan
        .dataset
        .sessions
        .iter()
        .map(|session| SessionMapping {
            legacy_session_record_id: session.id.clone(),
            legacy_host_id: session.host_id.clone(),
            legacy_host_session_id: session.host_session_id.clone(),
            session_id: deterministic_id("session", &session.id),
        })
        .collect();
    MigrationReport {
        report_schema: MIGRATION_REPORT_SCHEMA.to_owned(),
        mode: mode.to_owned(),
        source_schema: "v2".to_owned(),
        target_schema: "v3".to_owned(),
        started_at,
        finished_at,
        source_database_sha256: plan.inspected.source_database_sha256.clone(),
        counts: MigrationCounts {
            sessions_created: plan.dataset.sessions.len() as u64,
            waiting_requests_migrated: waiting,
            submitted_requests_migrated: submitted,
            drafts_migrated: drafts,
            artifacts_migrated: planned_artifact_count(&plan.dataset),
            records_dropped: plan.inspected.counts.records_dropped
                + plan.dataset.records_dropped_during_load,
        },
        session_mappings,
        losses: plan.dataset.losses.clone(),
        outputs,
    }
}

fn planned_artifact_count(dataset: &LegacyDataset) -> u64 {
    let mut digests = BTreeSet::new();
    for request in &dataset.requests {
        for file in &request.request_artifacts {
            digests.insert(bytes_digest(&file.bytes));
        }
        if request.waiting
            && let Some(draft) = &request.draft
        {
            for file in &draft.artifacts {
                digests.insert(bytes_digest(&file.bytes));
            }
        }
        if let Some(package) = &request.package {
            digests.insert(bytes_digest(&package.feedback.bytes));
            if let Some(uncooked) = &package.uncooked {
                digests.insert(bytes_digest(&uncooked.bytes));
            }
            for file in package
                .attachments
                .iter()
                .chain(package.request_attachments.iter())
            {
                digests.insert(bytes_digest(&file.bytes));
            }
        }
    }
    digests.len() as u64
}

async fn validate_new_target_root(target_root: &Path) -> Result<PathBuf, MigrationError> {
    if target_root.file_name().is_none() {
        return Err(MigrationError::InvalidTargetRoot);
    }
    match tokio::fs::symlink_metadata(target_root).await {
        Ok(_) => return Err(MigrationError::TargetExists),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(MigrationError::WriteTarget(error)),
    }
    let parent = target_root
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or(MigrationError::InvalidTargetParent)?;
    let parent_metadata = tokio::fs::symlink_metadata(parent)
        .await
        .map_err(|_| MigrationError::InvalidTargetParent)?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(MigrationError::InvalidTargetParent);
    }
    let parent = tokio::fs::canonicalize(parent)
        .await
        .map_err(|_| MigrationError::InvalidTargetParent)?;
    Ok(parent.join(
        target_root
            .file_name()
            .ok_or(MigrationError::InvalidTargetRoot)?,
    ))
}

fn staging_path(target_root: &Path) -> Result<PathBuf, MigrationError> {
    let parent = target_root
        .parent()
        .ok_or(MigrationError::InvalidTargetParent)?;
    let name = target_root
        .file_name()
        .ok_or(MigrationError::InvalidTargetRoot)?
        .to_string_lossy();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(parent.join(format!(".{name}.staging-{}-{nonce}", std::process::id())))
}

async fn copy_new_file(source: &Path, target: &Path) -> Result<(), MigrationError> {
    let mut source = tokio::fs::File::open(source)
        .await
        .map_err(MigrationError::WriteTarget)?;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut target = options
        .open(target)
        .await
        .map_err(MigrationError::WriteTarget)?;
    tokio::io::copy(&mut source, &mut target)
        .await
        .map_err(MigrationError::WriteTarget)?;
    target.sync_all().await.map_err(MigrationError::WriteTarget)
}

async fn make_read_only(path: &Path) -> Result<(), MigrationError> {
    let mut permissions = tokio::fs::metadata(path)
        .await
        .map_err(MigrationError::WriteTarget)?
        .permissions();
    permissions.set_readonly(true);
    tokio::fs::set_permissions(path, permissions)
        .await
        .map_err(MigrationError::WriteTarget)
}

async fn secure_directory(path: &Path) -> Result<(), MigrationError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(MigrationError::WriteTarget)?;
    }
    Ok(())
}

async fn make_tree_read_only(root: &Path) -> Result<(), MigrationError> {
    let mut paths = vec![root.to_path_buf()];
    let mut index = 0;
    while index < paths.len() {
        let current = paths[index].clone();
        index += 1;
        let metadata = tokio::fs::symlink_metadata(&current)
            .await
            .map_err(MigrationError::WriteTarget)?;
        if metadata.file_type().is_symlink() {
            return Err(MigrationError::Invariant(
                "backup tree contains a symlink".to_owned(),
            ));
        }
        if metadata.is_dir() {
            let mut entries = tokio::fs::read_dir(&current)
                .await
                .map_err(MigrationError::WriteTarget)?;
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(MigrationError::WriteTarget)?
            {
                paths.push(entry.path());
            }
        }
    }
    for path in paths.into_iter().rev() {
        let metadata = tokio::fs::symlink_metadata(&path)
            .await
            .map_err(MigrationError::WriteTarget)?;
        let mut permissions = metadata.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(permissions.mode() & !0o222);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(true);
        tokio::fs::set_permissions(&path, permissions)
            .await
            .map_err(MigrationError::WriteTarget)?;
    }
    Ok(())
}

async fn cleanup_staging(root: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut paths = vec![root.to_path_buf()];
        let mut index = 0;
        while index < paths.len() {
            let current = paths[index].clone();
            index += 1;
            if let Ok(metadata) = tokio::fs::symlink_metadata(&current).await {
                if metadata.is_dir() && !metadata.file_type().is_symlink() {
                    let _ = tokio::fs::set_permissions(
                        &current,
                        std::fs::Permissions::from_mode(0o700),
                    )
                    .await;
                    if let Ok(mut entries) = tokio::fs::read_dir(&current).await {
                        while let Ok(Some(entry)) = entries.next_entry().await {
                            paths.push(entry.path());
                        }
                    }
                } else if metadata.is_file() {
                    let _ = tokio::fs::set_permissions(
                        &current,
                        std::fs::Permissions::from_mode(0o600),
                    )
                    .await;
                }
            }
        }
    }
    let _ = tokio::fs::remove_dir_all(root).await;
}

async fn write_reports(root: &Path, report: &MigrationReport) -> Result<(), MigrationError> {
    let reports = root.join("reports");
    tokio::fs::create_dir(&reports)
        .await
        .map_err(MigrationError::WriteTarget)?;
    let mut json = serde_json::to_string_pretty(report).map_err(MigrationError::Serialize)?;
    json.push('\n');
    write_new_synced(&reports.join("migration-report.json"), json.as_bytes()).await?;
    let markdown = render_markdown(report);
    write_new_synced(&reports.join("migration-report.md"), markdown.as_bytes()).await?;
    Ok(())
}

pub(crate) fn render_markdown(report: &MigrationReport) -> String {
    let counts = &report.counts;
    let mut markdown = format!(
        "# RambleDesk v2 → v3 migration report\n\n- Mode: `{}`\n- Source digest: `{}`\n- Sessions created: {}\n- Waiting requests migrated: {}\n- Submitted requests migrated: {}\n- Drafts migrated: {}\n- Artifacts migrated: {}\n- Records dropped: {}\n\n## Losses\n\n",
        report.mode,
        report.source_database_sha256,
        counts.sessions_created,
        counts.waiting_requests_migrated,
        counts.submitted_requests_migrated,
        counts.drafts_migrated,
        counts.artifacts_migrated,
        counts.records_dropped,
    );
    if report.losses.is_empty() {
        markdown.push_str("None.\n");
    } else {
        for loss in &report.losses {
            markdown.push_str(&format!("- `{}`: `{}`\n", loss.legacy_id, loss.reason));
        }
    }
    markdown
}

async fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), MigrationError> {
    use tokio::io::AsyncWriteExt;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options
        .open(path)
        .await
        .map_err(MigrationError::WriteTarget)?;
    file.write_all(bytes)
        .await
        .map_err(MigrationError::WriteTarget)?;
    file.sync_all().await.map_err(MigrationError::WriteTarget)
}

async fn sync_tree(root: &Path) -> Result<(), MigrationError> {
    let mut directories = vec![root.to_path_buf()];
    let mut index = 0;
    while index < directories.len() {
        let current = directories[index].clone();
        index += 1;
        let mut entries = tokio::fs::read_dir(&current)
            .await
            .map_err(MigrationError::WriteTarget)?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(MigrationError::WriteTarget)?
        {
            let metadata = entry
                .metadata()
                .await
                .map_err(MigrationError::WriteTarget)?;
            if metadata.is_dir() {
                directories.push(entry.path());
            } else if metadata.is_file() {
                tokio::fs::File::open(entry.path())
                    .await
                    .map_err(MigrationError::WriteTarget)?
                    .sync_all()
                    .await
                    .map_err(MigrationError::WriteTarget)?;
            }
        }
    }
    for directory in directories.into_iter().rev() {
        sync_directory(&directory).await?;
    }
    Ok(())
}

async fn sync_directory(path: &Path) -> Result<(), MigrationError> {
    #[cfg(unix)]
    {
        tokio::fs::File::open(path)
            .await
            .map_err(MigrationError::WriteTarget)?
            .sync_all()
            .await
            .map_err(MigrationError::WriteTarget)?;
    }
    Ok(())
}

async fn publish_no_replace(source: &Path, target: &Path) -> Result<(), MigrationError> {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        let source = source.to_path_buf();
        let target = target.to_path_buf();
        let result = tokio::task::spawn_blocking(move || {
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                &source,
                rustix::fs::CWD,
                &target,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(std::io::Error::from)
        })
        .await
        .map_err(|error| MigrationError::Invariant(format!("publish task failed: {error}")))?;
        result.map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                MigrationError::TargetExists
            } else {
                MigrationError::WriteTarget(error)
            }
        })
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android",
        windows
    )))]
    {
        let _ = (source, target);
        Err(MigrationError::Invariant(
            "atomic no-replace publication is unsupported on this platform".to_owned(),
        ))
    }
    #[cfg(windows)]
    {
        let source = source.to_path_buf();
        let target = target.to_path_buf();
        let target_for_error = target.clone();
        let result = tokio::task::spawn_blocking(move || std::fs::rename(source, target))
            .await
            .map_err(|error| MigrationError::Invariant(format!("publish task failed: {error}")))?;
        result.map_err(|error| {
            if target_for_error.exists() {
                MigrationError::TargetExists
            } else {
                MigrationError::WriteTarget(error)
            }
        })
    }
}

fn now() -> String {
    let value = time::OffsetDateTime::now_utc();
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
