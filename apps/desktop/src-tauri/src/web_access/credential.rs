use std::{path::PathBuf, sync::Arc};

use rambledesk_local_server::DurableWebAccessToken;

use super::WebAccessCredentialStore;

pub(super) fn platform_store(app_data: PathBuf) -> Arc<dyn WebAccessCredentialStore> {
    #[cfg(unix)]
    return Arc::new(FileWebAccessCredentialStore::new(app_data.join("auth")));
    #[cfg(windows)]
    {
        let _ = app_data;
        Arc::new(WindowsWebAccessCredentialStore)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = app_data;
        Arc::new(UnavailableCredentialStore)
    }
}

#[cfg(unix)]
pub(super) struct FileWebAccessCredentialStore {
    directory: PathBuf,
}

#[cfg(unix)]
impl FileWebAccessCredentialStore {
    pub(super) fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    fn path(&self) -> PathBuf {
        self.directory.join("web-access.token")
    }

    fn prepare_directory(&self) -> std::io::Result<()> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

        // No OS credential interaction, including migration reads. Each bundle
        // identifier has its own app-data directory and independent credential.
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.directory)?;
        let metadata = std::fs::symlink_metadata(&self.directory)?;
        if !metadata.is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        std::fs::set_permissions(&self.directory, std::fs::Permissions::from_mode(0o700))
    }

    fn open_existing(&self) -> std::io::Result<std::fs::File> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

        // O_NONBLOCK also lets the metadata check reject a FIFO without waiting
        // for its writer. O_NOFOLLOW prevents reading/chmod-ing a linked target.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.path())?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        Ok(file)
    }

    fn read(&self) -> std::io::Result<DurableWebAccessToken> {
        use std::io::Read;

        let file = self.open_existing()?;
        let mut value = String::new();
        file.take(67).read_to_string(&mut value)?;
        if value.len() > 66 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        DurableWebAccessToken::parse(value.trim())
            .map_err(|_| std::io::ErrorKind::InvalidData.into())
    }

    fn write(&self, replace: bool) -> std::io::Result<DurableWebAccessToken> {
        use std::io::Write;

        let token = DurableWebAccessToken::generate();
        // NamedTempFile is owner-only on Unix. Publish only fully written bytes;
        // concurrent first starts either publish or read the same winning token.
        let mut candidate = tempfile::NamedTempFile::new_in(&self.directory)?;
        writeln!(candidate, "{}", token.secret())?;
        candidate.as_file().sync_all()?;
        if replace {
            candidate
                .persist(self.path())
                .map_err(|error| error.error)?;
        } else {
            match candidate.persist_noclobber(self.path()) {
                Ok(_) => {}
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return self.read();
                }
                Err(error) => return Err(error.error),
            }
        }
        Ok(token)
    }
}

#[cfg(unix)]
impl WebAccessCredentialStore for FileWebAccessCredentialStore {
    fn load_or_create(&self) -> Result<DurableWebAccessToken, String> {
        let load = || {
            self.prepare_directory()?;
            match self.read() {
                Ok(token) => Ok(token),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => self.write(false),
                Err(error) => Err(error),
            }
        };
        load().map_err(|_| credential_error())
    }

    fn rotate(&self) -> Result<DurableWebAccessToken, String> {
        let rotate = || {
            self.prepare_directory()?;
            match self.open_existing() {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            // Explicit rotation may replace malformed contents, but never follow
            // a symlink or remove a directory. Persistence precedes revocation.
            self.write(true)
        };
        rotate().map_err(|_| credential_error())
    }
}

#[cfg(windows)]
struct WindowsWebAccessCredentialStore;

#[cfg(windows)]
impl WindowsWebAccessCredentialStore {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(
            "com.rambledesk.desktop.web-access",
            "web-access-durable-token",
        )
        .map_err(|_| credential_error())
    }
}

#[cfg(windows)]
impl WebAccessCredentialStore for WindowsWebAccessCredentialStore {
    fn load_or_create(&self) -> Result<DurableWebAccessToken, String> {
        match Self::entry()?.get_password() {
            Ok(token) => DurableWebAccessToken::parse(token).map_err(|_| credential_error()),
            Err(keyring::Error::NoEntry) => self.rotate(),
            Err(_) => Err(credential_error()),
        }
    }

    fn rotate(&self) -> Result<DurableWebAccessToken, String> {
        let token = DurableWebAccessToken::generate();
        Self::entry()?
            .set_password(token.secret())
            .map_err(|_| credential_error())?;
        Ok(token)
    }
}

#[cfg(not(any(unix, windows)))]
struct UnavailableCredentialStore;

#[cfg(not(any(unix, windows)))]
impl WebAccessCredentialStore for UnavailableCredentialStore {
    fn load_or_create(&self) -> Result<DurableWebAccessToken, String> {
        Err(credential_error())
    }
    fn rotate(&self) -> Result<DurableWebAccessToken, String> {
        Err(credential_error())
    }
}

fn credential_error() -> String {
    "Web Access credential storage could not be read or written.".to_owned()
}

#[cfg(all(test, unix))]
mod tests;
