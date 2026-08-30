use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use rambledesk_core::kernel::{
    StoredBlob,
    ports::{ArtifactStore, ArtifactStoreError, PutArtifact},
};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

const DIGEST_PREFIX: &str = "sha256:";
const DIGEST_HEX_LEN: usize = 64;
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Filesystem-backed, content-addressed Artifact Store.
///
/// `open` accepts the RambleDesk library root. Bytes live below
/// `artifacts/sha256/<2>/<62>`; callers only receive the relative storage key.
#[derive(Clone)]
pub struct LocalArtifactStore {
    artifact_root: Arc<PathBuf>,
    sha256_root: Arc<PathBuf>,
    staging_root: Arc<PathBuf>,
    publish_lock: Arc<tokio::sync::Mutex<()>>,
}

impl LocalArtifactStore {
    pub async fn open(library_root: impl AsRef<Path>) -> Result<Self, ArtifactStoreError> {
        tokio::fs::create_dir_all(library_root.as_ref())
            .await
            .map_err(storage_error)?;
        let library_root = tokio::fs::canonicalize(library_root.as_ref())
            .await
            .map_err(storage_error)?;
        let artifact_root = library_root.join("artifacts");
        prepare_directory(&artifact_root).await?;
        let sha256_root = artifact_root.join("sha256");
        prepare_directory(&sha256_root).await?;
        let staging_root = artifact_root.join("staging");
        prepare_directory(&staging_root).await?;

        Ok(Self {
            artifact_root: Arc::new(artifact_root),
            sha256_root: Arc::new(sha256_root),
            staging_root: Arc::new(staging_root),
            publish_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    async fn ensure_layout(&self) -> Result<(), ArtifactStoreError> {
        ensure_real_directory(&self.artifact_root).await?;
        ensure_real_directory(&self.sha256_root).await?;
        ensure_real_directory(&self.staging_root).await
    }

    async fn prepare_shard(&self, prefix: &str) -> Result<PathBuf, ArtifactStoreError> {
        self.ensure_layout().await?;
        let shard = self.sha256_root.join(prefix);
        match tokio::fs::create_dir(&shard).await {
            Ok(()) => sync_directory(&self.sha256_root).await?,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(storage_error(error)),
        }
        ensure_real_directory(&shard).await?;
        Ok(shard)
    }

    async fn verify_blob(
        &self,
        path: &Path,
        expected_sha256: &str,
    ) -> Result<Vec<u8>, ArtifactStoreError> {
        let metadata = match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(ArtifactStoreError::NotFound);
            }
            Err(error) => return Err(storage_error(error)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ArtifactStoreError::Storage);
        }
        let contents = tokio::fs::read(path).await.map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                ArtifactStoreError::NotFound
            } else {
                storage_error(error)
            }
        })?;
        let actual_sha256 = sha256(&contents);
        if actual_sha256 != expected_sha256 {
            return Err(ArtifactStoreError::DigestMismatch);
        }
        Ok(contents)
    }

    fn blob_path(&self, storage_key: &str) -> Result<PathBuf, ArtifactStoreError> {
        let (_, prefix, suffix) = parse_storage_key(storage_key)?;
        Ok(self.sha256_root.join(prefix).join(suffix))
    }

    fn staging_path(&self) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let sequence = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        self.staging_root.join(format!(
            ".{}-{timestamp}-{sequence}.part",
            std::process::id()
        ))
    }

    async fn publish_staged(
        &self,
        staging_path: &Path,
        final_path: &Path,
        shard: &Path,
        expected_sha256: &str,
    ) -> Result<(), ArtifactStoreError> {
        let publication = match tokio::fs::hard_link(staging_path, final_path).await {
            Ok(()) => match sync_directory(shard).await {
                Ok(()) => self.verify_blob(final_path, expected_sha256).await,
                Err(error) => Err(error),
            },
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                // Another process won the publication race. Never replace its
                // entry; make the observed namespace durable and accept it
                // only if it contains the same immutable bytes.
                match sync_directory(shard).await {
                    Ok(()) => self.verify_blob(final_path, expected_sha256).await,
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(storage_error(error)),
        };

        let cleanup = match tokio::fs::remove_file(staging_path).await {
            Ok(()) => sync_directory(&self.staging_root).await,
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(storage_error(error)),
        };

        publication?;
        cleanup
    }
}

#[async_trait]
impl ArtifactStore for LocalArtifactStore {
    async fn put(&self, artifact: PutArtifact) -> Result<StoredBlob, ArtifactStoreError> {
        validate_digest(&artifact.expected_sha256)?;
        let actual_sha256 = sha256(&artifact.contents);
        if actual_sha256 != artifact.expected_sha256 {
            return Err(ArtifactStoreError::DigestMismatch);
        }
        let hex = actual_sha256
            .strip_prefix(DIGEST_PREFIX)
            .ok_or(ArtifactStoreError::DigestMismatch)?;
        let storage_key = format!("sha256/{}/{}", &hex[..2], &hex[2..]);
        let stored = StoredBlob {
            storage_key: storage_key.clone(),
            size_bytes: artifact.contents.len() as u64,
            sha256: actual_sha256.clone(),
        };

        let _guard = self.publish_lock.lock().await;
        let shard = self.prepare_shard(&hex[..2]).await?;
        let final_path = shard.join(&hex[2..]);
        match tokio::fs::symlink_metadata(&final_path).await {
            Ok(_) => {
                self.verify_blob(&final_path, &actual_sha256).await?;
                return Ok(stored);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(storage_error(error)),
        }

        let staging_path = self.staging_path();
        let publication = async {
            let mut options = tokio::fs::OpenOptions::new();
            options.write(true).create_new(true);
            let mut file = options.open(&staging_path).await.map_err(storage_error)?;
            file.write_all(&artifact.contents)
                .await
                .map_err(storage_error)?;
            file.flush().await.map_err(storage_error)?;
            file.sync_all().await.map_err(storage_error)?;
            drop(file);

            self.ensure_layout().await?;
            ensure_real_directory(&shard).await?;

            // `hard_link` is the cross-process no-clobber primitive: exactly
            // one publisher creates the final directory entry. Losers verify
            // the winner instead of replacing it.
            self.publish_staged(&staging_path, &final_path, &shard, &actual_sha256)
                .await
        }
        .await;

        if publication.is_err() && tokio::fs::remove_file(&staging_path).await.is_ok() {
            let _ = sync_directory(&self.staging_root).await;
        }
        publication?;
        Ok(stored)
    }

    async fn open_verified(
        &self,
        storage_key: &str,
        expected_sha256: &str,
    ) -> Result<Vec<u8>, ArtifactStoreError> {
        validate_digest(expected_sha256)?;
        let (key_digest, prefix, _) = parse_storage_key(storage_key)?;
        if key_digest != expected_sha256 {
            return Err(ArtifactStoreError::DigestMismatch);
        }
        self.ensure_layout().await?;
        let shard = self.sha256_root.join(prefix);
        match tokio::fs::symlink_metadata(&shard).await {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(ArtifactStoreError::Storage);
            }
            Ok(_) => ensure_real_directory(&shard).await?,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(ArtifactStoreError::NotFound);
            }
            Err(error) => return Err(storage_error(error)),
        }
        let path = self.blob_path(storage_key)?;
        self.verify_blob(&path, expected_sha256).await
    }
}

fn validate_digest(value: &str) -> Result<(), ArtifactStoreError> {
    let Some(hex) = value.strip_prefix(DIGEST_PREFIX) else {
        return Err(ArtifactStoreError::DigestMismatch);
    };
    if hex.len() != DIGEST_HEX_LEN
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ArtifactStoreError::DigestMismatch);
    }
    Ok(())
}

fn parse_storage_key(storage_key: &str) -> Result<(String, &str, &str), ArtifactStoreError> {
    if Path::new(storage_key).is_absolute() || storage_key.contains('\\') {
        return Err(ArtifactStoreError::Storage);
    }
    let mut parts = storage_key.split('/');
    let algorithm = parts.next();
    let prefix = parts.next();
    let suffix = parts.next();
    if algorithm != Some("sha256") || parts.next().is_some() {
        return Err(ArtifactStoreError::Storage);
    }
    let (Some(prefix), Some(suffix)) = (prefix, suffix) else {
        return Err(ArtifactStoreError::Storage);
    };
    let hex = format!("{prefix}{suffix}");
    let digest = format!("{DIGEST_PREFIX}{hex}");
    validate_digest(&digest).map_err(|_| ArtifactStoreError::Storage)?;
    if prefix.len() != 2 || suffix.len() != 62 {
        return Err(ArtifactStoreError::Storage);
    }
    Ok((digest, prefix, suffix))
}

async fn prepare_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    match tokio::fs::create_dir(path).await {
        Ok(()) => {
            if let Some(parent) = path.parent() {
                sync_directory(parent).await?;
            }
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(storage_error(error)),
    }
    ensure_real_directory(path).await
}

async fn ensure_real_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(storage_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ArtifactStoreError::Storage);
    }
    let canonical = tokio::fs::canonicalize(path).await.map_err(storage_error)?;
    if canonical != path {
        return Err(ArtifactStoreError::Storage);
    }
    Ok(())
}

#[cfg(unix)]
async fn sync_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    tokio::fs::File::open(path)
        .await
        .map_err(storage_error)?
        .sync_all()
        .await
        .map_err(storage_error)
}

#[cfg(not(unix))]
async fn sync_directory(_path: &Path) -> Result<(), ArtifactStoreError> {
    Ok(())
}

fn sha256(contents: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(contents)))
}

fn storage_error<T>(_error: T) -> ArtifactStoreError {
    ArtifactStoreError::Storage
}

#[cfg(test)]
mod tests;
