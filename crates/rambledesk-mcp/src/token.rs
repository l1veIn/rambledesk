use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use thiserror::Error;

const TOKEN_BYTES: usize = 32;
const TOKEN_HEX_LENGTH: usize = TOKEN_BYTES * 2;

#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(String);

impl AccessToken {
    pub fn generate() -> Self {
        let bytes: [u8; TOKEN_BYTES] = rand::random();
        Self(hex::encode(bytes))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, TokenError> {
        let value = value.into();
        if value.len() != TOKEN_HEX_LENGTH || !value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(TokenError::InvalidFormat);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn secret(&self) -> &str {
        &self.0
    }

    pub fn load_or_create(path: &Path) -> Result<Self, TokenError> {
        match read_token(path) {
            Ok(token) => return Ok(token),
            Err(TokenError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        let token = Self::generate();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        match options.open(path) {
            Ok(mut file) => {
                file.write_all(token.secret().as_bytes())?;
                file.write_all(b"\n")?;
                file.sync_all()?;
                Ok(token)
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => read_token(path),
            Err(error) => Err(error.into()),
        }
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken([REDACTED])")
    }
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("MCP access token must be exactly 64 hexadecimal characters")]
    InvalidFormat,
    #[error("failed to access MCP token file: {0}")]
    Io(#[from] io::Error),
    #[error("no local application data directory is available")]
    DataDirectoryUnavailable,
}

pub fn default_token_path() -> Result<PathBuf, TokenError> {
    dirs::data_local_dir()
        .map(|root| root.join("RambleDesk").join("auth").join("mcp.token"))
        .ok_or(TokenError::DataDirectoryUnavailable)
}

fn read_token(path: &Path) -> Result<AccessToken, TokenError> {
    AccessToken::parse(fs::read_to_string(path)?.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_have_256_bits_of_hex() {
        let token = AccessToken::generate();
        assert_eq!(token.secret().len(), TOKEN_HEX_LENGTH);
        assert!(token.secret().as_bytes().iter().all(u8::is_ascii_hexdigit));
        assert!(!format!("{token:?}").contains(token.secret()));
    }

    #[test]
    fn token_file_is_stable_across_reloads() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("auth").join("mcp.token");
        let first = AccessToken::load_or_create(&path).expect("create token");
        let second = AccessToken::load_or_create(&path).expect("reload token");
        assert_eq!(first, second);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).expect("metadata").permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn invalid_token_files_are_rejected() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("mcp.token");
        fs::write(&path, "short").expect("write fixture");
        assert!(matches!(
            AccessToken::load_or_create(&path),
            Err(TokenError::InvalidFormat)
        ));
    }
}
