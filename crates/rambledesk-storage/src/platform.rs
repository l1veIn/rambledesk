//! Platform filesystem durability primitives used by Feedback Package publication.
//!
//! The storage package owns publication semantics. This module is the only place
//! that translates those semantics into operating-system-specific filesystem
//! barriers.

use std::path::Path;

use rambledesk_core::RepositoryError;

pub(crate) async fn sync_staged_directory(path: &Path) -> Result<(), RepositoryError> {
    implementation::sync_staged_directory(path).await
}

pub(crate) async fn publish_directory(
    staged: &Path,
    published: &Path,
    parent: &Path,
) -> Result<(), RepositoryError> {
    implementation::publish_directory(staged, published, parent).await
}

pub(crate) async fn sync_published_parent(parent: &Path) -> Result<(), RepositoryError> {
    implementation::sync_published_parent(parent).await
}

#[cfg(unix)]
mod implementation {
    use super::*;

    pub(super) async fn sync_staged_directory(path: &Path) -> Result<(), RepositoryError> {
        sync_directory(path).await
    }

    pub(super) async fn publish_directory(
        staged: &Path,
        published: &Path,
        parent: &Path,
    ) -> Result<(), RepositoryError> {
        tokio::fs::rename(staged, published)
            .await
            .map_err(platform_error)?;
        sync_directory(parent).await
    }

    pub(super) async fn sync_published_parent(parent: &Path) -> Result<(), RepositoryError> {
        sync_directory(parent).await
    }

    async fn sync_directory(path: &Path) -> Result<(), RepositoryError> {
        tokio::fs::File::open(path)
            .await
            .map_err(platform_error)?
            .sync_all()
            .await
            .map_err(platform_error)
    }
}

#[cfg(windows)]
mod implementation {
    use std::{iter, os::windows::ffi::OsStrExt};

    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

    use super::*;

    pub(super) async fn sync_staged_directory(_path: &Path) -> Result<(), RepositoryError> {
        // Each package file is flushed before this point. Windows commits the
        // directory metadata together with the write-through move below.
        Ok(())
    }

    pub(super) async fn publish_directory(
        staged: &Path,
        published: &Path,
        _parent: &Path,
    ) -> Result<(), RepositoryError> {
        let staged = staged.to_path_buf();
        let published = published.to_path_buf();
        tokio::task::spawn_blocking(move || move_directory_write_through(&staged, &published))
            .await
            .map_err(platform_error)?
    }

    pub(super) async fn sync_published_parent(_parent: &Path) -> Result<(), RepositoryError> {
        // MoveFileExW with MOVEFILE_WRITE_THROUGH does not return until the
        // directory move has reached disk, so there is no second parent fsync.
        Ok(())
    }

    fn move_directory_write_through(
        staged: &Path,
        published: &Path,
    ) -> Result<(), RepositoryError> {
        let staged = wide_path(staged);
        let published = wide_path(published);
        let moved =
            unsafe { MoveFileExW(staged.as_ptr(), published.as_ptr(), MOVEFILE_WRITE_THROUGH) };
        if moved == 0 {
            return Err(RepositoryError::PackagePublish);
        }
        Ok(())
    }

    fn wide_path(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(iter::once(0))
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn write_through_move_publishes_a_directory_tree() {
            let root = tempfile::tempdir().expect("temporary root");
            let staged = root.path().join(".staged");
            let published = root.path().join("published");
            tokio::fs::create_dir(&staged)
                .await
                .expect("create staged directory");
            tokio::fs::write(staged.join("feedback.md"), b"feedback")
                .await
                .expect("write staged content");

            publish_directory(&staged, &published, root.path())
                .await
                .expect("publish directory");

            assert!(!staged.exists());
            assert_eq!(
                tokio::fs::read(published.join("feedback.md"))
                    .await
                    .expect("read published content"),
                b"feedback"
            );
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod implementation {
    use super::*;

    pub(super) async fn sync_staged_directory(_path: &Path) -> Result<(), RepositoryError> {
        Err(RepositoryError::PackagePublish)
    }

    pub(super) async fn publish_directory(
        _staged: &Path,
        _published: &Path,
        _parent: &Path,
    ) -> Result<(), RepositoryError> {
        Err(RepositoryError::PackagePublish)
    }

    pub(super) async fn sync_published_parent(_parent: &Path) -> Result<(), RepositoryError> {
        Err(RepositoryError::PackagePublish)
    }
}

fn platform_error<T>(_error: T) -> RepositoryError {
    RepositoryError::PackagePublish
}
