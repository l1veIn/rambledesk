use std::{collections::BTreeMap, path::Path};

use serde::Serialize;
use tokio::io::AsyncWriteExt;

use crate::{
    digest::bytes_digest,
    legacy_v2::{LegacyDataset, LegacyFile, LegacyPackage},
    migration::MigrationError,
};

#[derive(Debug, Clone)]
pub(crate) struct StoredArtifact {
    pub storage_key: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ArtifactIndex {
    by_digest: BTreeMap<String, StoredArtifact>,
}

impl ArtifactIndex {
    pub(crate) fn get(&self, digest: &str) -> Result<&StoredArtifact, MigrationError> {
        self.by_digest
            .get(digest)
            .ok_or_else(|| MigrationError::Invariant(format!("missing staged Artifact {digest}")))
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &StoredArtifact> {
        self.by_digest.values()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackupResult {
    pub object_count: u64,
}

#[derive(Debug, Serialize)]
struct BackupIndex {
    schema: &'static str,
    source_database: &'static str,
    source_database_sha256: String,
    objects: Vec<BackupIndexEntry>,
    session_mappings: Vec<BackupSessionMapping>,
}

#[derive(Debug, Serialize)]
struct BackupSessionMapping {
    legacy_session_record_id: String,
    legacy_host_id: String,
    legacy_host_session_id: String,
    session_id: String,
}

#[derive(Debug, Serialize)]
struct BackupIndexEntry {
    legacy_id: String,
    legacy_path: String,
    backup_object: String,
    sha256: String,
    size_bytes: u64,
}

struct ArtifactSource<'a> {
    legacy_id: String,
    legacy_path: String,
    bytes: &'a [u8],
    import_to_v3: bool,
}

pub(crate) async fn build_artifacts_and_backup(
    root: &Path,
    dataset: &LegacyDataset,
    source_database_sha256: &str,
) -> Result<(ArtifactIndex, BackupResult), MigrationError> {
    let library = root.join("library");
    let artifact_root = library.join("artifacts").join("sha256");
    let backup_objects = root
        .join("backup")
        .join("legacy-library")
        .join("objects")
        .join("sha256");
    tokio::fs::create_dir_all(&artifact_root)
        .await
        .map_err(MigrationError::WriteTarget)?;
    tokio::fs::create_dir_all(&backup_objects)
        .await
        .map_err(MigrationError::WriteTarget)?;
    let sources = collect_sources(dataset);
    let mut index = ArtifactIndex::default();
    let mut backup_entries = Vec::with_capacity(sources.len());
    for source in sources {
        let digest = bytes_digest(source.bytes);
        let (prefix, suffix) = digest_parts(&digest)?;
        let backup_key = format!("legacy-library/objects/sha256/{prefix}/{suffix}");
        write_content_addressed(&root.join("backup").join(&backup_key), source.bytes).await?;
        backup_entries.push(BackupIndexEntry {
            legacy_id: source.legacy_id,
            legacy_path: source.legacy_path,
            backup_object: backup_key,
            sha256: digest.clone(),
            size_bytes: source.bytes.len() as u64,
        });
        if source.import_to_v3 {
            let storage_key = format!("sha256/{prefix}/{suffix}");
            write_content_addressed(&library.join("artifacts").join(&storage_key), source.bytes)
                .await?;
            index
                .by_digest
                .entry(digest.clone())
                .or_insert(StoredArtifact {
                    storage_key,
                    sha256: digest,
                    size_bytes: source.bytes.len() as u64,
                });
        }
    }
    backup_entries.sort_by(|left, right| {
        left.legacy_id
            .cmp(&right.legacy_id)
            .then_with(|| left.legacy_path.cmp(&right.legacy_path))
    });
    let object_count = backup_entries
        .iter()
        .map(|entry| &entry.sha256)
        .collect::<std::collections::BTreeSet<_>>()
        .len() as u64;
    let sessions = dataset
        .sessions
        .iter()
        .map(|session| BackupSessionMapping {
            legacy_session_record_id: session.id.clone(),
            legacy_host_id: session.host_id.clone(),
            legacy_host_session_id: session.host_session_id.clone(),
            session_id: crate::digest::deterministic_id("session", &session.id),
        })
        .collect();
    let mut rendered = serde_json::to_string_pretty(&BackupIndex {
        schema: "rambledesk-v2-backup-index-v1",
        source_database: "source.sqlite3",
        source_database_sha256: source_database_sha256.to_owned(),
        objects: backup_entries,
        session_mappings: sessions,
    })
    .map_err(MigrationError::Serialize)?;
    rendered.push('\n');
    write_new_synced(
        &root
            .join("backup")
            .join("legacy-library")
            .join("index.json"),
        rendered.as_bytes(),
    )
    .await?;
    Ok((index, BackupResult { object_count }))
}

fn collect_sources(dataset: &LegacyDataset) -> Vec<ArtifactSource<'_>> {
    let mut sources = Vec::new();
    for file in &dataset.backup_files {
        sources.push(ArtifactSource {
            legacy_id: file.legacy_id.clone(),
            legacy_path: file.legacy_path.clone(),
            bytes: &file.bytes,
            import_to_v3: false,
        });
    }
    for request in &dataset.requests {
        for file in &request.request_artifacts {
            push_file(&mut sources, &request.id, file, true);
        }
        if request.waiting
            && let Some(draft) = &request.draft
        {
            for file in &draft.artifacts {
                push_file(&mut sources, &request.id, file, true);
            }
        }
        if let Some(package) = &request.package {
            push_package(&mut sources, &request.id, package);
        }
    }
    sources
}

fn push_file<'a>(
    sources: &mut Vec<ArtifactSource<'a>>,
    request_id: &str,
    file: &'a LegacyFile,
    import_to_v3: bool,
) {
    sources.push(ArtifactSource {
        legacy_id: format!("{request_id}:{}", file.id),
        legacy_path: file.legacy_path.clone(),
        bytes: &file.bytes,
        import_to_v3,
    });
}

fn push_package<'a>(
    sources: &mut Vec<ArtifactSource<'a>>,
    request_id: &str,
    package: &'a LegacyPackage,
) {
    for artifact in std::iter::once(&package.manifest)
        .chain(std::iter::once(&package.feedback))
        .chain(package.uncooked.iter())
        .chain(package.attachments.iter())
        .chain(package.request_attachments.iter())
    {
        sources.push(ArtifactSource {
            legacy_id: format!("{request_id}:{}", artifact.id),
            legacy_path: artifact.legacy_path.clone(),
            bytes: &artifact.bytes,
            import_to_v3: artifact.id != package.manifest.id,
        });
    }
}

async fn write_content_addressed(path: &Path, bytes: &[u8]) -> Result<(), MigrationError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(MigrationError::WriteTarget)?;
    }
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    match options.open(path).await {
        Ok(mut file) => {
            file.write_all(bytes)
                .await
                .map_err(MigrationError::WriteTarget)?;
            file.sync_all().await.map_err(MigrationError::WriteTarget)?;
            drop(file);
            if let Some(parent) = path.parent() {
                sync_directory(parent).await?;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = tokio::fs::symlink_metadata(path)
                .await
                .map_err(MigrationError::WriteTarget)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(MigrationError::Invariant(
                    "content-addressed object is not a real file".to_owned(),
                ));
            }
            let existing = tokio::fs::read(path)
                .await
                .map_err(MigrationError::WriteTarget)?;
            if existing == bytes {
                Ok(())
            } else {
                Err(MigrationError::Invariant(
                    "content-addressed object collision".to_owned(),
                ))
            }
        }
        Err(error) => Err(MigrationError::WriteTarget(error)),
    }
}

async fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), MigrationError> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options
        .open(path)
        .await
        .map_err(MigrationError::WriteTarget)?;
    file.write_all(bytes)
        .await
        .map_err(MigrationError::WriteTarget)?;
    file.sync_all().await.map_err(MigrationError::WriteTarget)?;
    if let Some(parent) = path.parent() {
        sync_directory(parent).await?;
    }
    Ok(())
}

async fn sync_directory(path: &Path) -> Result<(), MigrationError> {
    #[cfg(unix)]
    tokio::fs::File::open(path)
        .await
        .map_err(MigrationError::WriteTarget)?
        .sync_all()
        .await
        .map_err(MigrationError::WriteTarget)?;
    Ok(())
}

fn digest_parts(digest: &str) -> Result<(&str, &str), MigrationError> {
    let hex = digest
        .strip_prefix("sha256:")
        .filter(|value| value.len() == 64)
        .ok_or_else(|| MigrationError::Invariant("invalid Artifact digest".to_owned()))?;
    Ok((&hex[..2], &hex[2..]))
}
