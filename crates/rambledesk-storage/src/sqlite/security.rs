use super::*;

#[cfg(unix)]
pub(super) async fn secure_new_path(
    path: &Path,
    existed: bool,
    mode: u32,
) -> Result<(), StorageOpenError> {
    if existed {
        return Ok(());
    }
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .await
        .map_err(StorageOpenError::SecurePath)
}

#[cfg(not(unix))]
pub(super) async fn secure_new_path(
    _path: &Path,
    _existed: bool,
    _mode: u32,
) -> Result<(), StorageOpenError> {
    Ok(())
}

#[cfg(unix)]
pub(super) async fn secure_path(path: &Path, mode: u32) -> Result<(), StorageOpenError> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .await
        .map_err(StorageOpenError::SecurePath)
}

#[cfg(not(unix))]
pub(super) async fn secure_path(_path: &Path, _mode: u32) -> Result<(), StorageOpenError> {
    Ok(())
}
