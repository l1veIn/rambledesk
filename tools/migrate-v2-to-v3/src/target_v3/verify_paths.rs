use std::{io::ErrorKind, path::Path};

use sha2::{Digest, Sha256};

use crate::migration::MigrationError;

pub(super) async fn read_real_file(root: &Path, path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| std::io::Error::new(ErrorKind::InvalidInput, "path outside target root"))?;
    let canonical_root = tokio::fs::canonicalize(root).await?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(std::io::Error::new(ErrorKind::InvalidInput, "unsafe path"));
        }
        current.push(component);
        let metadata = tokio::fs::symlink_metadata(&current).await?;
        if metadata.file_type().is_symlink() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "symlink in path",
            ));
        }
    }
    let metadata = tokio::fs::symlink_metadata(&current).await?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "not a real file",
        ));
    }
    let canonical = tokio::fs::canonicalize(&current).await?;
    if !canonical.starts_with(canonical_root) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "path escaped target root",
        ));
    }
    tokio::fs::read(canonical).await
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

pub(super) async fn reject_target_sidecars(database: &Path) -> Result<(), MigrationError> {
    let name = database
        .file_name()
        .ok_or(MigrationError::InvalidTargetRoot)?
        .to_string_lossy();
    for suffix in ["wal", "shm"] {
        let sidecar = database.with_file_name(format!("{name}-{suffix}"));
        match tokio::fs::symlink_metadata(sidecar).await {
            Ok(metadata) if metadata.file_type().is_symlink() || metadata.len() > 0 => {
                return Err(MigrationError::TargetActiveWal);
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(MigrationError::WriteTarget(error)),
        }
    }
    Ok(())
}
