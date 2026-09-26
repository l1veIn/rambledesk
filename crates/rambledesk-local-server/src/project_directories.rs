use std::{
    fs, io,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use rambledesk_core::{
    BrowseProjectDirectoriesInput, ProjectDirectoryBrowser, ProjectDirectoryEntry,
    ProjectDirectoryError, ProjectDirectoryErrorCode, ProjectDirectoryListing,
};

/// Read-only host filesystem adapter. Authentication is owned by the application
/// router; filesystem access retains the process user's existing OS permissions.
pub struct LocalProjectDirectoryBrowser;

#[async_trait]
impl ProjectDirectoryBrowser for LocalProjectDirectoryBrowser {
    async fn browse(
        &self,
        input: BrowseProjectDirectoriesInput,
    ) -> Result<ProjectDirectoryListing, ProjectDirectoryError> {
        tokio::task::spawn_blocking(move || browse(input))
            .await
            .map_err(|_| {
                ProjectDirectoryError::new(ProjectDirectoryErrorCode::DirectoryUnavailable)
            })?
    }
}

fn browse(
    input: BrowseProjectDirectoriesInput,
) -> Result<ProjectDirectoryListing, ProjectDirectoryError> {
    let home = dirs::home_dir().and_then(|path| fs::canonicalize(path).ok());
    let path = match input.path {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err(ProjectDirectoryError::new(
                    ProjectDirectoryErrorCode::InvalidDirectoryPath,
                ));
            }
            path
        }
        None => home
            .clone()
            .or_else(|| roots().first().map(|entry| PathBuf::from(&entry.path)))
            .ok_or_else(|| {
                ProjectDirectoryError::new(ProjectDirectoryErrorCode::DirectoryUnavailable)
            })?,
    };
    let path = fs::canonicalize(path).map_err(directory_error)?;
    if !fs::metadata(&path).map_err(directory_error)?.is_dir() {
        return Err(ProjectDirectoryError::new(
            ProjectDirectoryErrorCode::InvalidDirectoryPath,
        ));
    }
    let mut directories = Vec::new();
    for entry in fs::read_dir(&path).map_err(directory_error)? {
        let entry = entry.map_err(directory_error)?;
        let kind = match entry.file_type() {
            Ok(kind) => kind,
            Err(_) => continue,
        };
        if !(kind.is_dir() || kind.is_symlink() && entry.path().is_dir()) {
            continue;
        }
        // Do not emit lossy paths that could select another filesystem entry.
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(path) = display_path(&entry.path()) else {
            continue;
        };
        directories.push(ProjectDirectoryEntry { name, path });
    }
    directories.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(ProjectDirectoryListing {
        path: display_path(&path).ok_or_else(|| {
            ProjectDirectoryError::new(ProjectDirectoryErrorCode::InvalidDirectoryPath)
        })?,
        parent_path: path.parent().and_then(display_path),
        home_path: home.as_deref().and_then(display_path),
        roots: roots(),
        directories,
    })
}

fn directory_error(error: io::Error) -> ProjectDirectoryError {
    let code = match error.kind() {
        io::ErrorKind::NotFound => ProjectDirectoryErrorCode::DirectoryNotFound,
        io::ErrorKind::PermissionDenied => ProjectDirectoryErrorCode::DirectoryAccessDenied,
        io::ErrorKind::InvalidInput | io::ErrorKind::NotADirectory => {
            ProjectDirectoryErrorCode::InvalidDirectoryPath
        }
        _ => ProjectDirectoryErrorCode::DirectoryUnavailable,
    };
    ProjectDirectoryError::new(code)
}

fn display_path(path: &Path) -> Option<String> {
    let value = path.to_str()?;
    #[cfg(windows)]
    {
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            return Some(format!(r"\\{unc}"));
        }
        if let Some(drive) = value.strip_prefix(r"\\?\") {
            return Some(drive.into());
        }
    }
    Some(value.into())
}

fn roots() -> Vec<ProjectDirectoryEntry> {
    #[cfg(windows)]
    {
        // This enumerates drives without probing disconnected network volumes.
        let drives = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        (0..26)
            .filter(|index| drives & (1 << index) != 0)
            .map(|index| {
                let path = format!("{}:\\", char::from(b'A' + index));
                ProjectDirectoryEntry {
                    name: path.clone(),
                    path,
                }
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        vec![ProjectDirectoryEntry {
            name: "/".into(),
            path: "/".into(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_only_immediate_directories_in_name_order_and_normalizes_paths() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["zeta", "Alpha", "beta", ".hidden"] {
            fs::create_dir(temp.path().join(name)).unwrap();
        }
        fs::write(temp.path().join("private.txt"), "not returned").unwrap();
        fs::create_dir(temp.path().join("Alpha").join("nested")).unwrap();
        let listing = LocalProjectDirectoryBrowser
            .browse(BrowseProjectDirectoriesInput {
                path: Some(
                    temp.path()
                        .join("Alpha")
                        .join("..")
                        .to_str()
                        .unwrap()
                        .into(),
                ),
            })
            .await
            .unwrap();
        assert_eq!(
            listing.path,
            display_path(&fs::canonicalize(temp.path()).unwrap()).unwrap()
        );
        assert_eq!(
            listing
                .directories
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            [".hidden", "Alpha", "beta", "zeta"]
        );
        assert!(listing.parent_path.is_some());
        assert!(!listing.roots.is_empty());
        for entry in listing.directories {
            assert!(Path::new(&entry.path).is_absolute());
        }
    }

    #[test]
    fn rejects_missing_files_relative_paths_and_invalid_input() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("file"), "").unwrap();
        for path in [
            "relative".into(),
            "".into(),
            temp.path().join("file").to_str().unwrap().into(),
        ] {
            assert_eq!(
                browse(BrowseProjectDirectoriesInput { path: Some(path) })
                    .unwrap_err()
                    .code,
                ProjectDirectoryErrorCode::InvalidDirectoryPath
            );
        }
        assert_eq!(
            browse(BrowseProjectDirectoriesInput {
                path: Some(temp.path().join("missing").to_str().unwrap().into())
            })
            .unwrap_err()
            .code,
            ProjectDirectoryErrorCode::DirectoryNotFound
        );
    }

    #[test]
    fn default_location_and_root_navigation_are_usable() {
        let listing = browse(BrowseProjectDirectoriesInput { path: None }).unwrap();
        assert!(Path::new(&listing.path).is_absolute());
        let root = Path::new(&listing.path).ancestors().last().unwrap();
        let listing = browse(BrowseProjectDirectoriesInput {
            path: display_path(root),
        })
        .unwrap();
        assert_eq!(listing.parent_path, None);
    }

    #[test]
    fn permission_errors_remain_distinct_without_raw_os_details() {
        let error = directory_error(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private diagnostic",
        ));
        assert_eq!(error.code, ProjectDirectoryErrorCode::DirectoryAccessDenied);
        assert!(!error.message.contains("private diagnostic"));
        assert!(!error.retryable);
    }
}
