use super::StorageOpenError;

pub fn default_app_data_root() -> Result<std::path::PathBuf, StorageOpenError> {
    dirs::data_local_dir()
        .map(|root| root.join("RambleDesk"))
        .ok_or(StorageOpenError::DataDirectoryUnavailable)
}

pub fn default_database_path() -> Result<std::path::PathBuf, StorageOpenError> {
    default_app_data_root().map(|root| root.join("state").join("feedback.sqlite3"))
}

pub fn default_library_path() -> Result<std::path::PathBuf, StorageOpenError> {
    default_app_data_root().map(|root| root.join("library"))
}
