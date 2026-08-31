use std::{
    fs,
    io::{self, Cursor},
    path::{Component, Path},
    process::Stdio,
};

use flate2::read::GzDecoder;
use futures::StreamExt as _;
use rambledesk_acp_client::{BuiltinAgentDistribution, BuiltinAgentSpec, PlatformArtifact};
use sha2::{Digest as _, Sha256};
use tokio::{io::AsyncWriteExt as _, process::Command};

use super::super::model::AcpWorkbenchError;

const NPM_REGISTRY: &str = "https://registry.npmjs.org";

pub(super) async fn install_npm(
    prefix: &Path,
    package: &str,
    expected_command: &Path,
) -> Result<(), AcpWorkbenchError> {
    let npm = super::resolve_executable("npm").ok_or_else(|| {
        AcpWorkbenchError::new(
            "ACP_RUNTIME_MISSING",
            "npm is required to install this ACP Agent client",
            false,
        )
    })?;
    fs::create_dir_all(prefix).map_err(install_io_error)?;
    let mut command = Command::new(npm);
    command
        .arg("install")
        .arg("--prefix")
        .arg(prefix)
        .args(["--no-audit", "--no-fund", "--include=optional"])
        .arg(format!("--registry={NPM_REGISTRY}"))
        .arg(package)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = command.output().await.map_err(install_io_error)?;
    if !output.status.success() || !expected_command.is_file() {
        let detail = bounded_install_detail(&output.stderr);
        return Err(AcpWorkbenchError::new(
            "ACP_INSTALL_FAILED",
            format!(
                "npm did not install {package} at the expected managed command (status {}){detail}",
                output
                    .status
                    .code()
                    .map_or_else(|| "signal".to_owned(), |code| code.to_string())
            ),
            true,
        ));
    }
    Ok(())
}

fn bounded_install_detail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let mut detail = text
        .lines()
        .rev()
        .find(|line| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with("npm error A complete log")
        })
        .unwrap_or_default()
        .trim()
        .to_owned();
    detail.truncate(500);
    if detail.is_empty() {
        String::new()
    } else {
        format!(": {detail}")
    }
}

pub(super) async fn install_binary(
    install_dir: &Path,
    expected_command: &Path,
    artifact: &PlatformArtifact,
    spec: &BuiltinAgentSpec,
) -> Result<(), AcpWorkbenchError> {
    let parent = install_dir.parent().ok_or_else(|| {
        AcpWorkbenchError::new("ACP_INSTALL_FAILED", "invalid managed install path", false)
    })?;
    fs::create_dir_all(parent).map_err(install_io_error)?;
    let staging = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(parent)
        .map_err(install_io_error)?;
    let archive_path = staging.path().join("agent-download");
    download(artifact, &archive_path).await?;
    let extraction = staging.path().join("tree");
    fs::create_dir(&extraction).map_err(install_io_error)?;
    extract_archive(&archive_path, artifact.url, &extraction)?;
    let staged_command = extraction.join(
        expected_command
            .strip_prefix(install_dir)
            .map_err(install_io_error)?,
    );
    validate_binary_install(&staged_command, &extraction, spec)?;
    make_binary_install_executable(&staged_command, &extraction, spec)?;
    if install_dir.exists() {
        fs::remove_dir_all(install_dir).map_err(install_io_error)?;
    }
    fs::rename(&extraction, install_dir).map_err(install_io_error)?;
    validate_binary_install(expected_command, install_dir, spec)
}

async fn download(
    artifact: &PlatformArtifact,
    destination: &Path,
) -> Result<(), AcpWorkbenchError> {
    let response = reqwest::Client::new()
        .get(artifact.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| {
            AcpWorkbenchError::new(
                "ACP_INSTALL_FAILED",
                format!("could not download the pinned Agent client: {error}"),
                true,
            )
        })?;
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(install_io_error)?;
    let mut digest = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            AcpWorkbenchError::new(
                "ACP_INSTALL_FAILED",
                format!("Agent client download was interrupted: {error}"),
                true,
            )
        })?;
        digest.update(&chunk);
        file.write_all(&chunk).await.map_err(install_io_error)?;
    }
    file.flush().await.map_err(install_io_error)?;
    if let Some(expected) = artifact.sha256 {
        let actual = hex::encode(digest.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(AcpWorkbenchError::new(
                "ACP_INSTALL_FAILED",
                "the downloaded Agent client failed its checksum verification",
                true,
            ));
        }
    }
    Ok(())
}

fn extract_archive(archive: &Path, url: &str, destination: &Path) -> Result<(), AcpWorkbenchError> {
    if url.ends_with(".zip") {
        let bytes = fs::read(archive).map_err(install_io_error)?;
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(install_io_error)?;
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(install_io_error)?;
            let relative = entry.enclosed_name().ok_or_else(|| {
                AcpWorkbenchError::new(
                    "ACP_INSTALL_FAILED",
                    "the Agent archive contains an unsafe path",
                    false,
                )
            })?;
            let output = destination.join(relative);
            if entry.is_dir() {
                fs::create_dir_all(&output).map_err(install_io_error)?;
                continue;
            }
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(install_io_error)?;
            }
            let mut file = fs::File::create(&output).map_err(install_io_error)?;
            io::copy(&mut entry, &mut file).map_err(install_io_error)?;
        }
        return Ok(());
    }
    if url.ends_with(".tar.gz") {
        let file = fs::File::open(archive).map_err(install_io_error)?;
        let mut tar = tar::Archive::new(GzDecoder::new(file));
        for item in tar.entries().map_err(install_io_error)? {
            let mut entry = item.map_err(install_io_error)?;
            let path = entry.path().map_err(install_io_error)?;
            if path.components().any(|part| {
                matches!(
                    part,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            }) {
                return Err(AcpWorkbenchError::new(
                    "ACP_INSTALL_FAILED",
                    "the Agent archive contains an unsafe path",
                    false,
                ));
            }
            entry.unpack_in(destination).map_err(install_io_error)?;
        }
        return Ok(());
    }
    Err(AcpWorkbenchError::new(
        "ACP_INSTALL_FAILED",
        "the pinned Agent client uses an unsupported archive format",
        false,
    ))
}

pub(super) fn validate_binary_install(
    executable: &Path,
    install_dir: &Path,
    spec: &BuiltinAgentSpec,
) -> Result<(), AcpWorkbenchError> {
    if !executable.is_file() {
        return Err(AcpWorkbenchError::new(
            "ACP_INSTALL_FAILED",
            format!(
                "{} executable is missing from its managed install",
                spec.label
            ),
            true,
        ));
    }
    let BuiltinAgentDistribution::Binary {
        directory_entry: Some(entry),
        ..
    } = spec.distribution
    else {
        return Ok(());
    };
    let siblings = if cfg!(windows) {
        entry.required_siblings.windows
    } else {
        entry.required_siblings.unix
    };
    for sibling in siblings {
        if !install_dir.join(sibling).is_file() {
            return Err(AcpWorkbenchError::new(
                "ACP_INSTALL_FAILED",
                format!("{} managed install is missing {sibling}", spec.label),
                true,
            ));
        }
    }
    Ok(())
}

fn make_binary_install_executable(
    executable: &Path,
    install_dir: &Path,
    spec: &BuiltinAgentSpec,
) -> Result<(), AcpWorkbenchError> {
    make_executable(executable)?;
    let BuiltinAgentDistribution::Binary {
        directory_entry: Some(entry),
        ..
    } = spec.distribution
    else {
        return Ok(());
    };
    let siblings = if cfg!(windows) {
        entry.required_siblings.windows
    } else {
        entry.required_siblings.unix
    };
    for sibling in siblings {
        make_executable(&install_dir.join(sibling))?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), AcpWorkbenchError> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = fs::metadata(path).map_err(install_io_error)?.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    fs::set_permissions(path, permissions).map_err(install_io_error)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), AcpWorkbenchError> {
    Ok(())
}

pub(super) fn install_io_error(error: impl std::fmt::Display) -> AcpWorkbenchError {
    AcpWorkbenchError::new(
        "ACP_INSTALL_FAILED",
        format!("could not prepare the managed Agent client: {error}"),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_archive_paths_are_rejected() {
        assert!(
            Path::new("../escape")
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        );
    }

    #[cfg(unix)]
    #[test]
    fn binary_tree_helpers_are_made_executable_with_the_main_entry() {
        use std::os::unix::fs::PermissionsExt as _;

        let spec =
            rambledesk_acp_client::builtin_agent("antigravity").expect("Antigravity catalog entry");
        let root = tempfile::tempdir().expect("tempdir");
        let main = root.path().join("agy_acp_server.par");
        let helper = root.path().join("localharness_external");
        fs::write(&main, "server").expect("main");
        fs::write(&helper, "helper").expect("helper");

        make_binary_install_executable(&main, root.path(), spec).expect("chmod tree");

        assert_ne!(fs::metadata(main).unwrap().permissions().mode() & 0o111, 0);
        assert_ne!(
            fs::metadata(helper).unwrap().permissions().mode() & 0o111,
            0
        );
    }
}
