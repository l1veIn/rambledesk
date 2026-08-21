//! One-click installation of the Pi-native RambleDesk package.
//!
//! The Pi adapter is a Pi package (`packages/pi-rambledesk`) that talks to the
//! authenticated loopback JSON API directly and waits inside the Pi tool call,
//! so no post-submit continuation strategy is needed. Installing it is a plain
//! `pi install <package dir>` invocation against either bundled application
//! resources or a local source checkout.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use rambledesk_core::find_executable;
use serde::Serialize;
use serde_json::Value;

const RAMBLEDESK_PI_PACKAGE_NAME: &str = "@rambledesk/pi";

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
/// binaries are resolvable in this crate. macOS GUI applications inherit a
/// minimal launchd PATH instead of the user's shell PATH, so also inspect the
/// common Homebrew, npm/pnpm and Node version-manager locations.
pub fn resolve_pi_binary(home: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RAMBLEDESK_PI_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(path) = find_executable("pi") {
        return Some(path);
    }
    fallback_pi_candidates(home)
        .into_iter()
        .find(|candidate| candidate.is_file())
}

fn fallback_pi_candidates(home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/opt/homebrew/bin/pi"));
        candidates.push(PathBuf::from("/usr/local/bin/pi"));
    }
    let Some(home) = home else {
        return candidates;
    };
    candidates.extend([
        home.join(".local/bin/pi"),
        home.join("Library/pnpm/pi"),
        home.join(".npm-global/bin/pi"),
        home.join(".bun/bin/pi"),
        home.join(".volta/bin/pi"),
        home.join(".asdf/shims/pi"),
        home.join(".local/share/mise/shims/pi"),
        home.join(".proto/shims/pi"),
    ]);
    append_version_manager_candidates(&mut candidates, &home.join(".nvm/versions/node"));
    append_version_manager_candidates(
        &mut candidates,
        &home.join(".local/share/fnm/node-versions"),
    );
    candidates
}

fn append_version_manager_candidates(candidates: &mut Vec<PathBuf>, root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut directories = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    directories.sort_by(|left, right| right.cmp(left));
    for directory in directories {
        // nvm uses <version>/bin; fnm uses <version>/installation/bin.
        candidates.push(directory.join("bin/pi"));
        candidates.push(directory.join("installation/bin/pi"));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiPackageStatus {
    pub cli_available: bool,
    pub installed: bool,
    pub source_count: usize,
    pub restart_required: bool,
}

/// Inspect Pi's user settings without modifying them.
pub fn package_status(
    home: &Path,
    current_package_dir: Option<&Path>,
) -> Result<PiPackageStatus, String> {
    let sources = rambledesk_pi_sources(home, current_package_dir)?;
    Ok(PiPackageStatus {
        cli_available: resolve_pi_binary(Some(home)).is_some(),
        installed: !sources.is_empty(),
        source_count: sources.len(),
        restart_required: true,
    })
}

fn pi_agent_dir(home: &Path) -> PathBuf {
    std::env::var_os("PI_CODING_AGENT_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".pi").join("agent"))
}

fn rambledesk_pi_sources(
    home: &Path,
    current_package_dir: Option<&Path>,
) -> Result<Vec<String>, String> {
    let agent_dir = pi_agent_dir(home);
    let settings_path = agent_dir.join("settings.json");
    if !settings_path.is_file() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&settings_path).map_err(|error| {
        format!(
            "Could not read Pi settings at {}: {error}",
            settings_path.display()
        )
    })?;
    let settings: Value = serde_json::from_str(&content).map_err(|error| {
        format!(
            "Could not parse Pi settings at {}: {error}",
            settings_path.display()
        )
    })?;
    let Some(packages) = settings.get("packages") else {
        return Ok(Vec::new());
    };
    let packages = packages.as_array().ok_or_else(|| {
        format!(
            "Pi settings at {} contain an invalid packages field",
            settings_path.display()
        )
    })?;
    Ok(packages
        .iter()
        .filter_map(package_source_string)
        .filter(|source| is_rambledesk_pi_source(source, &agent_dir, current_package_dir))
        .map(str::to_owned)
        .collect())
}

fn package_source_string(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("source").and_then(Value::as_str))
}

fn is_rambledesk_pi_source(
    source: &str,
    agent_dir: &Path,
    current_package_dir: Option<&Path>,
) -> bool {
    if let Some(spec) = source.strip_prefix("npm:") {
        return spec == RAMBLEDESK_PI_PACKAGE_NAME
            || spec
                .strip_prefix(RAMBLEDESK_PI_PACKAGE_NAME)
                .is_some_and(|suffix| suffix.starts_with('@'));
    }
    if source.starts_with("git:")
        || source.starts_with("git@")
        || source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("ssh://")
    {
        return false;
    }

    let source_path = absolute_local_source(source, agent_dir);
    if current_package_dir.is_some_and(|current| paths_match(&source_path, current)) {
        return true;
    }
    let manifest_path = source_path.join("package.json");
    if manifest_path.is_file() {
        return std::fs::read_to_string(manifest_path)
            .ok()
            .and_then(|content| serde_json::from_str::<Value>(&content).ok())
            .and_then(|package| {
                package
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .is_some_and(|name| name == RAMBLEDESK_PI_PACKAGE_NAME);
    }
    // Keep stale RambleDesk registrations removable after an old local package
    // directory or app bundle has already disappeared.
    source_path
        .file_name()
        .is_some_and(|name| name == "pi-rambledesk")
}

fn absolute_local_source(source: &str, agent_dir: &Path) -> PathBuf {
    let path = PathBuf::from(source);
    if path.is_absolute() {
        path
    } else {
        agent_dir.join(path)
    }
}

fn paths_match(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// Run `pi install <package_dir>` and return the tail of its output.
pub fn run_install(
    pi_bin: &Path,
    package_dir: &Path,
    home: Option<&Path>,
) -> Result<String, String> {
    let package_dir = path_for_pi(package_dir);
    run_package_command(pi_bin, "install", package_dir.as_os_str(), home)
}

/// Remove every user-level Pi package source that resolves to RambleDesk.
pub fn run_uninstall(
    pi_bin: &Path,
    home: &Path,
    current_package_dir: Option<&Path>,
) -> Result<String, String> {
    let sources = rambledesk_pi_sources(home, current_package_dir)?;
    let agent_dir = pi_agent_dir(home);
    let mut details = Vec::new();
    for source in sources {
        let command_source = if source.starts_with("npm:") {
            OsString::from(&source)
        } else {
            absolute_local_source(&source, &agent_dir).into_os_string()
        };
        let detail = run_package_command(pi_bin, "remove", &command_source, Some(home))?;
        if !detail.is_empty() {
            details.push(detail);
        }
    }
    Ok(details.join("\n"))
}

fn run_package_command(
    pi_bin: &Path,
    action: &str,
    source: &std::ffi::OsStr,
    home: Option<&Path>,
) -> Result<String, String> {
    let mut command = Command::new(pi_bin);
    command.arg(action).arg(source);
    if let Some(path) = installer_search_path(pi_bin, home) {
        // npm/pnpm commonly install `pi` as a `#!/usr/bin/env node` script.
        // Finder-launched macOS applications do not inherit the user's shell
        // PATH, so finding the absolute `pi` script is not enough: `env` must
        // also be able to find the adjacent Node runtime. Preserve the
        // inherited PATH after the known package-manager/version-manager dirs.
        command.env("PATH", path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // The packaged app is a GUI process without a console. Spawning the pi
        // shim (a console `.cmd`/node process) would otherwise flash a black
        // console window for the whole install; CREATE_NO_WINDOW suppresses it.
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let output = command
        .output()
        .map_err(|error| format!("Failed to run `pi {action}`: {error}"))?;
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
            format!("`pi {action}` exited with {}", output.status)
        } else {
            format!("`pi {action}` exited with {}: {detail}", output.status)
        })
    }
}

fn installer_search_path(pi_bin: &Path, home: Option<&Path>) -> Option<OsString> {
    let mut directories = Vec::new();
    if let Some(parent) = pi_bin.parent() {
        push_unique(&mut directories, parent.to_path_buf());
    }
    for candidate in fallback_pi_candidates(home) {
        if let Some(parent) = candidate.parent() {
            push_unique(&mut directories, parent.to_path_buf());
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            push_unique(&mut directories, directory);
        }
    }
    std::env::join_paths(directories).ok()
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
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

    #[test]
    fn fallback_candidates_include_user_package_managers() {
        let home = Path::new("/tmp/rambledesk-test-home");
        let candidates = fallback_pi_candidates(Some(home));
        assert!(candidates.contains(&home.join(".local/bin/pi")));
        assert!(candidates.contains(&home.join("Library/pnpm/pi")));
        assert!(candidates.contains(&home.join(".volta/bin/pi")));
    }

    #[test]
    fn package_detection_finds_npm_local_and_filtered_sources() {
        let home =
            std::env::temp_dir().join(format!("rambledesk-pi-status-{}", std::process::id()));
        let agent_dir = home.join(".pi/agent");
        let local_package = agent_dir.join("packages/pi-rambledesk");
        std::fs::create_dir_all(&local_package).unwrap();
        std::fs::write(
            local_package.join("package.json"),
            r#"{"name":"@rambledesk/pi"}"#,
        )
        .unwrap();
        std::fs::write(
            agent_dir.join("settings.json"),
            r#"{"packages":["npm:@rambledesk/pi",{"source":"packages/pi-rambledesk","extensions":["index.js"]},"npm:another-package"]}"#,
        )
        .unwrap();

        assert_eq!(
            rambledesk_pi_sources(&home, None).unwrap(),
            vec![
                "npm:@rambledesk/pi".to_owned(),
                "packages/pi-rambledesk".to_owned()
            ]
        );

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn missing_pi_settings_report_not_installed() {
        let home = std::env::temp_dir().join(format!(
            "rambledesk-pi-status-missing-{}",
            std::process::id()
        ));
        assert!(rambledesk_pi_sources(&home, None).unwrap().is_empty());
    }

    #[test]
    fn version_manager_candidates_include_nvm_and_fnm_layouts() {
        let root = std::env::temp_dir().join(format!(
            "rambledesk-pi-version-manager-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("v22.0.0")).unwrap();
        let mut candidates = Vec::new();
        append_version_manager_candidates(&mut candidates, &root);
        assert!(candidates.contains(&root.join("v22.0.0/bin/pi")));
        assert!(candidates.contains(&root.join("v22.0.0/installation/bin/pi")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_fallback_candidates_include_homebrew() {
        let candidates = fallback_pi_candidates(None);
        assert!(candidates.contains(&PathBuf::from("/opt/homebrew/bin/pi")));
        assert!(candidates.contains(&PathBuf::from("/usr/local/bin/pi")));
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
            run_install(&pi_bin, &package_dir, None).unwrap(),
            "adapter installed"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn run_install_exposes_adjacent_runtime_to_env_shebang() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("rambledesk-pi-env-runtime-{}", std::process::id()));
        let bin_dir = root.join("bin");
        let package_dir = root.join("package");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::create_dir_all(&package_dir).unwrap();

        let runtime = bin_dir.join("rambledesk-test-node");
        std::fs::write(
            &runtime,
            "#!/bin/sh\nscript=\"$1\"\nshift\nexec /bin/sh \"$script\" \"$@\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755)).unwrap();

        let pi_bin = bin_dir.join("pi");
        std::fs::write(
            &pi_bin,
            "#!/usr/bin/env rambledesk-test-node\n[ \"$1\" = install ] || exit 41\necho adapter installed\n",
        )
        .unwrap();
        std::fs::set_permissions(&pi_bin, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(
            run_install(&pi_bin, &package_dir, None).unwrap(),
            "adapter installed"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn run_uninstall_removes_every_detected_rambledesk_source() {
        use std::os::unix::fs::PermissionsExt;

        let home =
            std::env::temp_dir().join(format!("rambledesk-pi-uninstall-{}", std::process::id()));
        let agent_dir = home.join(".pi/agent");
        let local_package = agent_dir.join("packages/pi-rambledesk");
        std::fs::create_dir_all(&local_package).unwrap();
        std::fs::write(
            local_package.join("package.json"),
            r#"{"name":"@rambledesk/pi"}"#,
        )
        .unwrap();
        std::fs::write(
            agent_dir.join("settings.json"),
            r#"{"packages":["npm:@rambledesk/pi","packages/pi-rambledesk"]}"#,
        )
        .unwrap();

        let pi_bin = home.join("pi");
        std::fs::write(&pi_bin, "#!/bin/sh\necho \"$1 $2\"\n").unwrap();
        std::fs::set_permissions(&pi_bin, std::fs::Permissions::from_mode(0o755)).unwrap();

        let output = run_uninstall(&pi_bin, &home, None).unwrap();
        assert!(output.contains("remove npm:@rambledesk/pi"));
        assert!(output.contains(&format!("remove {}", local_package.display())));

        let _ = std::fs::remove_dir_all(&home);
    }
}
