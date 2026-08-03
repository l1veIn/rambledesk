//! One-click installation of the Pi-native RambleDesk package.
//!
//! The Pi adapter is a Pi package (`packages/pi-rambledesk`) that talks to the
//! authenticated loopback JSON API directly and waits inside the Pi tool call,
//! so no post-submit continuation strategy is needed. Installing it is a plain
//! `pi install <package dir>` invocation against either bundled application
//! resources or a local source checkout.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::platform::process::find_executable;

/// Locate the Pi package directory.
///
/// Candidates are tried in order:
/// 1. `<checkout_root>/packages/pi-rambledesk` for an explicit development checkout,
/// 2. a source checkout above `<resource_dir>` (Tauri's development resource
///    directory is under `target/debug`),
/// 3. `<resource_dir>/pi-rambledesk` for a packaged desktop application, then
/// 4. a directory walk upward from the current working directory and the
///    current executable, looking for a sibling `packages/pi-rambledesk`.
pub fn resolve_package_dir(
    checkout_root: Option<&str>,
    resource_dir: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(root) = checkout_root
        && !root.is_empty()
        && PathBuf::from(root)
            .join("packages")
            .join("pi-rambledesk")
            .join("package.json")
            .is_file()
    {
        return Some(PathBuf::from(root).join("packages").join("pi-rambledesk"));
    }
    if let Some(resource_dir) = resource_dir {
        // In `tauri dev`, resources are copied under `target/debug`. Prefer the
        // source package above that directory so a developer who already ran
        // `pi install ./packages/pi-rambledesk` does not register a second copy.
        if let Some(checkout_package) = find_package_dir_from(resource_dir) {
            return Some(checkout_package);
        }
        let bundled = resource_dir.join("pi-rambledesk");
        if bundled.join("package.json").is_file() {
            return Some(bundled);
        }
    }
    let mut anchors: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        anchors.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        anchors.push(parent.to_path_buf());
    }
    anchors
        .into_iter()
        .find_map(|anchor| find_package_dir_from(&anchor))
}

/// Walk upward from `anchor` looking for a sibling `packages/pi-rambledesk`.
fn find_package_dir_from(anchor: &Path) -> Option<PathBuf> {
    let mut probe = anchor.to_path_buf();
    for _ in 0..6 {
        let candidate = probe.join("packages").join("pi-rambledesk");
        if candidate.join("package.json").is_file() {
            return Some(candidate);
        }
        if !probe.pop() {
            break;
        }
    }
    None
}

/// Locate the `pi` CLI binary.
///
/// `RAMBLEDESK_PI_BIN` overrides PATH lookup, mirroring how other adapter
/// binaries are resolvable in this crate.
pub fn resolve_pi_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RAMBLEDESK_PI_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    find_executable("pi")
}

/// Run `pi install <package_dir>` and return the tail of its output.
pub fn run_install(pi_bin: &Path, package_dir: &Path) -> Result<String, String> {
    let package_dir = path_for_pi(package_dir);
    let output = Command::new(pi_bin)
        .arg("install")
        .arg(&package_dir)
        .output()
        .map_err(|error| format!("Failed to run `pi install`: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let tail = stdout
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(tail)
    } else {
        let detail = stderr.trim();
        Err(if detail.is_empty() {
            format!("`pi install` exited with {}", output.status)
        } else {
            format!("`pi install` exited with {}: {detail}", output.status)
        })
    }
}

/// Pi persists local package arguments verbatim. Tauri may return Windows
/// resource paths with the `\\?\` extended-length prefix; it addresses the same
/// file but is noisy in the UI and is treated as a different package identity
/// from the ordinary drive-letter spelling.
fn path_for_pi(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        dunce::simplified(path).to_path_buf()
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_repo(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("rambledesk-{label}-{}", std::process::id()));
        let pkg = root.join("packages").join("pi-rambledesk");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("package.json"), "{}").unwrap();
        root
    }

    #[test]
    fn package_dir_resolves_from_checkout_root() {
        let root = fake_repo("pi-install-root");
        let expected = root.join("packages").join("pi-rambledesk");
        assert_eq!(
            resolve_package_dir(Some(root.to_str().unwrap()), None),
            Some(expected.clone())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn package_dir_resolves_from_bundled_resources() {
        let root =
            std::env::temp_dir().join(format!("rambledesk-pi-resources-{}", std::process::id()));
        let resource_dir = root.join("resources");
        let bundled = resource_dir.join("pi-rambledesk");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::write(bundled.join("package.json"), "{}").unwrap();

        assert_eq!(
            resolve_package_dir(None, Some(&resource_dir)),
            Some(bundled)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn development_resources_prefer_the_source_checkout() {
        let root = fake_repo("pi-install-dev-resources");
        let resource_dir = root.join("target").join("debug");
        let bundled = resource_dir.join("pi-rambledesk");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::write(bundled.join("package.json"), "{}").unwrap();

        assert_eq!(
            resolve_package_dir(None, Some(&resource_dir)),
            Some(root.join("packages").join("pi-rambledesk"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn package_dir_walks_up_from_nested_anchor() {
        let root = fake_repo("pi-install-walk");
        let anchor = root.join("apps").join("desktop");
        std::fs::create_dir_all(&anchor).unwrap();
        assert_eq!(
            find_package_dir_from(&anchor),
            Some(root.join("packages").join("pi-rambledesk"))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn package_dir_missing_returns_none() {
        // A bare temp directory with no packages/pi-rambledesk above it must not resolve.
        let root =
            std::env::temp_dir().join(format!("rambledesk-pi-install-none-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(find_package_dir_from(&root), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn pi_path_removes_windows_verbatim_prefix() {
        assert_eq!(
            path_for_pi(Path::new(r"\\?\C:\Users\Test\pi-rambledesk")),
            PathBuf::from(r"C:\Users\Test\pi-rambledesk")
        );
    }

    #[cfg(windows)]
    #[test]
    fn run_install_executes_windows_cmd_shim() {
        let root =
            std::env::temp_dir().join(format!("rambledesk pi command {}", std::process::id()));
        let package_dir = root.join("package with spaces");
        std::fs::create_dir_all(&package_dir).unwrap();
        let pi_bin = root.join("pi.cmd");
        std::fs::write(
            &pi_bin,
            "@echo off\r\nif not \"%~1\"==\"install\" exit /b 41\r\necho adapter installed\r\n",
        )
        .unwrap();

        assert_eq!(
            run_install(&pi_bin, &package_dir).unwrap(),
            "adapter installed"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
