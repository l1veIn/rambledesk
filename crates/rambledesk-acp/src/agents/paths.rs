// Adapted from Codeg 3ebdfed acp/binary_cache.rs (Apache-2.0): publish only complete
// installations and clean temporary trees. Changed: immutable npm generations,
// atomic current pointer, dedicated-root marker, and containment before deletion.
use super::{CatalogError, runner::CommandSpec};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncReadExt;

const MARKER: &str = "RambleDesk managed Agent packages v1\n";

pub(super) async fn is_managed_generation(prefix: &Path, id: &str) -> bool {
    let Some(generation) = prefix.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if uuid::Uuid::parse_str(generation).is_err() {
        return false;
    }
    let Some(versions) = prefix.parent() else {
        return false;
    };
    let Some(agent) = versions.parent() else {
        return false;
    };
    let Some(root) = agent.parent() else {
        return false;
    };
    versions.file_name().is_some_and(|name| name == "versions")
        && agent.file_name().is_some_and(|name| name == id)
        && tokio::fs::read_to_string(root.join(".rambledesk-agents"))
            .await
            .ok()
            .as_deref()
            == Some(MARKER)
}

// Rust canonicalization uses verbatim Windows paths. Node's module resolver does
// not reliably accept those as CLI arguments; keep canonical paths for boundary
// checks and use ordinary absolute paths when crossing the subprocess boundary.
pub(crate) fn command_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            return rest.into();
        }
    }
    value.into_owned()
}

pub(super) async fn prepare_root(root: &Path) -> Result<PathBuf, CatalogError> {
    if root.exists() {
        let marker = tokio::fs::read_to_string(root.join(".rambledesk-agents"))
            .await
            .ok();
        if marker.as_deref() != Some(MARKER) {
            let mut entries = tokio::fs::read_dir(root)
                .await
                .map_err(|_| CatalogError::InvalidRoot)?;
            if entries
                .next_entry()
                .await
                .map_err(|_| CatalogError::InvalidRoot)?
                .is_some()
            {
                return Err(CatalogError::InvalidRoot);
            }
        }
    }
    tokio::fs::create_dir_all(root)
        .await
        .map_err(|_| CatalogError::Storage)?;
    tokio::fs::write(root.join(".rambledesk-agents"), MARKER)
        .await
        .map_err(|_| CatalogError::Storage)?;
    tokio::fs::canonicalize(root)
        .await
        .map_err(|_| CatalogError::Storage)
}

pub(super) async fn json(path: &Path) -> Result<serde_json::Value, CatalogError> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| CatalogError::InvalidInstall)?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| CatalogError::InvalidInstall)?;
    if bytes.len() > 1024 * 1024 {
        return Err(CatalogError::InvalidInstall);
    }
    serde_json::from_slice(&bytes).map_err(|_| CatalogError::InvalidInstall)
}

pub(super) async fn contained(root: &Path, path: &Path) -> Result<PathBuf, CatalogError> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map_err(|_| CatalogError::InvalidInstall)?;
    if canonical == root || !canonical.starts_with(root) {
        return Err(CatalogError::InvalidInstall);
    }
    Ok(canonical)
}

pub(super) async fn directory(root: &Path, path: &Path) -> Result<PathBuf, CatalogError> {
    let mut existing = path;
    while !existing.exists() {
        existing = existing.parent().ok_or(CatalogError::InvalidRoot)?;
    }
    let ancestor = tokio::fs::canonicalize(existing)
        .await
        .map_err(|_| CatalogError::InvalidRoot)?;
    if !ancestor.starts_with(root) {
        return Err(CatalogError::InvalidRoot);
    }
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|_| CatalogError::Storage)?;
    contained(root, path).await
}

pub(super) async fn package(
    prefix: &Path,
    name: &str,
    command: &str,
    node: &Path,
) -> Result<(String, CommandSpec), CatalogError> {
    let (version, binary) = package_entry(prefix, name, command).await?;
    Ok((version, launch(&binary, node).await?))
}

/// Package discovery is independent of its runtime being installed.
pub(super) async fn package_entry(
    prefix: &Path,
    name: &str,
    command: &str,
) -> Result<(String, PathBuf), CatalogError> {
    let root = tokio::fs::canonicalize(prefix)
        .await
        .map_err(|_| CatalogError::InvalidInstall)?;
    let directory = contained(&root, &prefix.join("node_modules").join(name)).await?;
    let meta = json(&directory.join("package.json")).await?;
    if meta["name"] != name {
        return Err(CatalogError::InvalidInstall);
    }
    let version = meta["version"]
        .as_str()
        .and_then(super::version::sanitize)
        .ok_or(CatalogError::InvalidInstall)?;
    let bin = meta["bin"]
        .as_str()
        .or_else(|| meta["bin"][command].as_str())
        .ok_or(CatalogError::InvalidInstall)?;
    let relative = Path::new(bin);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(CatalogError::InvalidInstall);
    }
    let binary = contained(&root, &directory.join(relative)).await?;
    if !binary.is_file() {
        return Err(CatalogError::InvalidInstall);
    }
    Ok((version, binary))
}

pub(super) async fn requires_node(path: &Path) -> Result<bool, CatalogError> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| CatalogError::CommandUnavailable)?;
    let mut first = [0u8; 4096];
    let count = file
        .read(&mut first)
        .await
        .map_err(|_| CatalogError::CommandUnavailable)?;
    let text = String::from_utf8_lossy(&first[..count]);
    Ok(matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("js" | "mjs" | "cjs")
    ) || (first.starts_with(b"#!")
        && text
            .lines()
            .next()
            .is_some_and(|line| line.contains("node")))
        // npm shims can still be runnable without npm itself. Recognize their
        // Node runtime even when a package metadata lookup was not possible.
        || (matches!(path.extension().and_then(|value| value.to_str()), Some("cmd" | "bat"))
            && (text.contains("node.exe") || text.contains("_prog=node")))
        || (first.starts_with(b"#!/bin/sh") && text.contains("node") && text.contains("basedir")))
}

pub(super) async fn launch(path: &Path, node: &Path) -> Result<CommandSpec, CatalogError> {
    let path = tokio::fs::canonicalize(path)
        .await
        .map_err(|_| CatalogError::CommandUnavailable)?;
    let script = requires_node(&path).await?;
    if script && !node.is_file() {
        return Err(CatalogError::CommandUnavailable);
    }
    let mut header = [0u8; 16];
    tokio::fs::File::open(&path)
        .await
        .map_err(|_| CatalogError::CommandUnavailable)?
        .read(&mut header)
        .await
        .map_err(|_| CatalogError::CommandUnavailable)?;
    let shim = matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("cmd" | "bat")
    ) || (script && header.starts_with(b"#!/bin/sh"));
    if script && !shim {
        Ok(CommandSpec {
            command: command_path(node),
            args: vec![command_path(&path)],
        })
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if tokio::fs::metadata(&path)
                .await
                .map_err(|_| CatalogError::InvalidInstall)?
                .permissions()
                .mode()
                & 0o111
                == 0
            {
                return Err(CatalogError::InvalidInstall);
            }
        }
        Ok(CommandSpec {
            command: command_path(&path),
            args: vec![],
        })
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct Current {
    pub generation: String,
}

pub(super) async fn current(root: &Path, id: &str) -> Result<Option<PathBuf>, CatalogError> {
    let pointer = root.join(id).join("current.json");
    if !pointer.exists() {
        return Ok(None);
    }
    let current: Current =
        serde_json::from_value(json(&pointer).await?).map_err(|_| CatalogError::InvalidInstall)?;
    if uuid::Uuid::parse_str(&current.generation).is_err() {
        return Err(CatalogError::InvalidInstall);
    }
    let root = tokio::fs::canonicalize(root)
        .await
        .map_err(|_| CatalogError::InvalidInstall)?;
    Ok(Some(
        contained(
            &root,
            &root.join(id).join("versions").join(current.generation),
        )
        .await?,
    ))
}

pub(super) struct Staging {
    pub root: PathBuf,
    pub path: PathBuf,
    pub published: bool,
}
impl Staging {
    pub async fn clean(&mut self) -> Result<(), CatalogError> {
        if !self.path.exists() {
            self.published = true;
            return Ok(());
        }
        let path = contained(&self.root, &self.path).await?;
        tokio::fs::remove_dir_all(path)
            .await
            .map_err(|_| CatalogError::Storage)?;
        self.published = true;
        Ok(())
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        let root = self.root.clone();
        let path = self.path.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                for _ in 0..8 {
                    if !path.exists() {
                        break;
                    }
                    if let Ok(target) = contained(&root, &path).await {
                        if tokio::fs::remove_dir_all(target).await.is_ok() {
                            break;
                        }
                    } else {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            });
        }
    }
}
