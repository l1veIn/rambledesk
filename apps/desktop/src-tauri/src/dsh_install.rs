//! One-click installation of the dsh-native RambleDesk plugin.
//!
//! The dsh adapter is a Cordis plugin (`packages/dsh-rambledesk`) that talks to
//! the authenticated loopback JSON API directly and waits inside the dsh tool
//! call, so no post-submit continuation strategy is needed. Installing it is:
//!
//! 1. copy the plugin package next to a dsh profile
//!    (`<profile>/plugins/rambledesk`),
//! 2. append the loader entry to the profile's `cordis.patch.yml` (idempotent),
//! 3. install the shared `ramble` skill into the user's global skill directory
//!    (`~/.agents/skills/ramble/SKILL.md`), which dsh's skill-filesystem
//!    provider discovers.
//!
//! The patch file is a top-level YAML array; row order carries no load
//! semantics, so appending a fresh `- insert:` block is always safe. dsh
//! resolves relative plugin specifiers against the profile directory, so the
//! plugin is addressed as `./plugins/rambledesk/index.js`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::Manager;

use rambledesk_core::find_executable;
use rambledesk_hosts::RAMBLE_SKILL_MD;

const PATCH_ID: &str = "rambledesk";
const PLUGIN_SUBDIR: &str = "plugins/rambledesk";
const SKILL_TARGET_RELATIVE: &str = ".agents/skills/ramble/SKILL.md";
const PATCH_ENTRY: &str = "- insert:\n    - id: rambledesk\n      name: './plugins/rambledesk/index.js'\n      config:\n        hostId: dsh\n";

/// dsh home directory. `DSH_HOME` overrides the default `~/.dsh`.
pub fn dsh_home(home: &Path) -> PathBuf {
    std::env::var_os("DSH_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".dsh"))
}

/// One dsh profile that can host the RambleDesk plugin.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshProfileView {
    /// Profile display name (the directory name under `profiles/`).
    pub id: String,
    /// Absolute path of the profile directory (the loader's `baseUrl`).
    pub profile_dir: String,
    /// Absolute path of the profile's `cordis.patch.yml`.
    pub patch_path: String,
    /// Whether the patch file already contains the RambleDesk entry.
    pub configured: bool,
}

/// Result of installing the plugin into one dsh profile.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshInstallResult {
    pub profile_id: String,
    pub profile_dir: String,
    pub patch_path: String,
    pub action: &'static str,
    pub restart_required: bool,
}

/// Detection view for the dsh host, mirroring `McpHostView`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshHostView {
    pub id: &'static str,
    pub name: &'static str,
    pub installed: bool,
    pub profiles: Vec<DshProfileView>,
    pub restart_required: bool,
}

/// Detect dsh on this device: whether the CLI is present and which profiles
/// can host the RambleDesk plugin.
pub fn scan_dsh_host(home: &Path) -> DshHostView {
    let profiles = list_dsh_profiles(home);
    DshHostView {
        id: "dsh",
        name: "DeepSeek Harness",
        installed: resolve_dsh_binary().is_some() || !profiles.is_empty(),
        profiles,
        restart_required: true,
    }
}

/// Enumerate dsh profiles that expose a `cordis.patch.yml`.
pub fn list_dsh_profiles(home: &Path) -> Vec<DshProfileView> {
    let profiles_dir = dsh_home(home).join("profiles");
    let Ok(entries) = fs::read_dir(&profiles_dir) else {
        return Vec::new();
    };
    let mut profiles = Vec::new();
    for entry in entries.flatten() {
        let profile_dir = entry.path();
        if !profile_dir.is_dir() {
            continue;
        }
        let patch_path = profile_dir.join("cordis.patch.yml");
        if !patch_path.is_file() {
            continue;
        }
        let configured = fs::read_to_string(&patch_path)
            .is_ok_and(|content| content.contains(&format!("id: {PATCH_ID}")));
        profiles.push(DshProfileView {
            id: entry.file_name().to_string_lossy().into_owned(),
            profile_dir: profile_dir.to_string_lossy().into_owned(),
            patch_path: patch_path.to_string_lossy().into_owned(),
            configured,
        });
    }
    profiles.sort_by(|left, right| left.id.cmp(&right.id));
    profiles
}

/// Whether the `dsh` CLI is available on PATH (or `RAMBLEDESK_DSH_BIN`).
pub fn resolve_dsh_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RAMBLEDESK_DSH_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    find_executable("dsh")
}

/// Locate the dsh plugin package directory.
///
/// Candidates mirror `pi_install::resolve_package_dir`:
/// 1. `<checkout_root>/packages/dsh-rambledesk` for an explicit development checkout,
/// 2. a source checkout above `<resource_dir>`,
/// 3. `<resource_dir>/dsh-rambledesk` for a packaged desktop application, then
/// 4. a directory walk upward from the current working directory and the
///    current executable, looking for a sibling `packages/dsh-rambledesk`.
pub fn resolve_package_dir(
    checkout_root: Option<&str>,
    resource_dir: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(root) = checkout_root
        && !root.is_empty()
        && PathBuf::from(root)
            .join("packages")
            .join("dsh-rambledesk")
            .join("package.json")
            .is_file()
    {
        return Some(PathBuf::from(root).join("packages").join("dsh-rambledesk"));
    }
    if let Some(resource_dir) = resource_dir {
        if let Some(checkout_package) = find_package_dir_from(resource_dir) {
            return Some(checkout_package);
        }
        let bundled = resource_dir.join("dsh-rambledesk");
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

/// Walk upward from `anchor` looking for a sibling `packages/dsh-rambledesk`.
fn find_package_dir_from(anchor: &Path) -> Option<PathBuf> {
    let mut probe = anchor.to_path_buf();
    for _ in 0..6 {
        let candidate = probe.join("packages").join("dsh-rambledesk");
        if candidate.join("package.json").is_file() {
            return Some(candidate);
        }
        if !probe.pop() {
            break;
        }
    }
    None
}

/// Install the RambleDesk plugin into one dsh profile.
pub fn install_dsh(
    home: &Path,
    profile: &DshProfileView,
    package_dir: &Path,
) -> Result<DshInstallResult, String> {
    let profile_dir = PathBuf::from(&profile.profile_dir);
    let plugin_target = profile_dir.join(PLUGIN_SUBDIR);

    copy_plugin_package(package_dir, &plugin_target)?;

    let patch_path = PathBuf::from(&profile.patch_path);
    let patch_action = append_patch_entry(&patch_path)?;

    let skill_action = install_ramble_skill(home)?;

    let action = if patch_action == "unchanged" && skill_action == "unchanged" {
        "unchanged"
    } else if patch_action == "created" {
        "created"
    } else {
        "updated"
    };
    Ok(DshInstallResult {
        profile_id: profile.id.clone(),
        profile_dir: profile.profile_dir.clone(),
        patch_path: profile.patch_path.clone(),
        action,
        restart_required: true,
    })
}

/// Copy the plugin package files into `<profile>/plugins/rambledesk`.
/// Byte-equal files are left untouched (idempotent).
fn copy_plugin_package(package_dir: &Path, target: &Path) -> Result<(), String> {
    let entries = fs::read_dir(package_dir)
        .map_err(|error| {
            format!(
                "Could not read plugin package {}: {error}",
                package_dir.display()
            )
        })?
        .flatten();
    let mut copied = false;
    for entry in entries {
        let file_name = entry.file_name();
        let source = entry.path();
        if source.is_dir() {
            continue;
        }
        if !["index.js", "package.json", "README.md"]
            .contains(&file_name.to_string_lossy().as_ref())
        {
            continue;
        }
        copied |= copy_if_changed(&source, &target.join(&file_name))?;
    }
    if copied {
        tracing::info!(target = %target.display(), "copied dsh plugin package");
    }
    Ok(())
}

/// Append the loader entry to `cordis.patch.yml` when absent. Returns the
/// resulting action: "unchanged", "updated", or "created".
///
/// The patch file is ONE top-level YAML array of patch entries. An empty
/// array (`[]`, possibly behind leading comments) must therefore be REPLACED
/// by the insert block — appending after it would open a second top-level
/// element and make the file unparseable. A non-empty list appends another
/// `- insert:` element.
fn append_patch_entry(patch_path: &Path) -> Result<&'static str, String> {
    let existed = patch_path.exists();
    let content = if existed {
        fs::read_to_string(patch_path)
            .map_err(|error| format!("Could not read {}: {error}", patch_path.display()))?
    } else {
        String::new()
    };
    if content.contains(&format!("id: {PATCH_ID}")) {
        return Ok("unchanged");
    }
    let trimmed_end = content.trim_end();
    let updated = if trimmed_end.ends_with("[]") {
        // The trailing empty array must be REPLACED by the insert block
        // (keeping any leading comments); appending after it would open a
        // second top-level element and make the file unparseable.
        let head = trimmed_end
            .strip_suffix("[]")
            .expect("ends_with checked above");
        format!("{head}{PATCH_ENTRY}")
    } else if content.trim().is_empty() {
        PATCH_ENTRY.to_owned()
    } else {
        let separator = if content.ends_with('\n') { "" } else { "\n" };
        format!("{content}{separator}{PATCH_ENTRY}")
    };
    fs::write(patch_path, updated)
        .map_err(|error| format!("Could not write {}: {error}", patch_path.display()))?;
    Ok(if existed { "updated" } else { "created" })
}

/// Install the canonical `ramble` skill into `~/.agents/skills`.
fn install_ramble_skill(home: &Path) -> Result<&'static str, String> {
    let target = home.join(SKILL_TARGET_RELATIVE);
    let existed = target.exists();
    if fs::read_to_string(&target).is_ok_and(|current| current == RAMBLE_SKILL_MD) {
        return Ok("unchanged");
    }
    let parent = target
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    fs::write(&target, RAMBLE_SKILL_MD)
        .map_err(|error| format!("Could not write {}: {error}", target.display()))?;
    Ok(if existed { "updated" } else { "created" })
}

/// Write `source` to `target` when the byte content differs. Creates parent
/// directories. Returns whether a write happened.
fn copy_if_changed(source: &Path, target: &Path) -> Result<bool, String> {
    let content = fs::read(source)
        .map_err(|error| format!("Could not read {}: {error}", source.display()))?;
    let unchanged = fs::read(target).is_ok_and(|existing| existing == content);
    if unchanged {
        return Ok(false);
    }
    let parent = target
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    fs::write(target, content)
        .map_err(|error| format!("Could not write {}: {error}", target.display()))?;
    Ok(true)
}

// #region tauri commands

/// Detection command for the Settings → Adapters dsh card.
#[tauri::command]
pub(super) fn detect_dsh_host(app: tauri::AppHandle) -> Result<DshHostView, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    Ok(scan_dsh_host(&home))
}

/// Install the RambleDesk plugin into every (or one selected) dsh profile.
#[tauri::command]
pub(super) async fn install_dsh_package(
    app: tauri::AppHandle,
    checkout_root: Option<String>,
    profile_id: Option<String>,
) -> Result<Vec<DshInstallResult>, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("Could not resolve bundled application resources: {error}"))?;
    let package_dir = resolve_package_dir(checkout_root.as_deref(), Some(&resource_dir))
        .ok_or_else(|| {
            "Could not locate the bundled dsh-rambledesk package. Reinstall RambleDesk or copy `packages/dsh-rambledesk` into the dsh profile plugins directory manually."
                .to_owned()
        })?;
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Could not resolve the user home directory: {error}"))?;
    let profiles = list_dsh_profiles(&home);
    if profiles.is_empty() {
        return Err(
            "No dsh profiles found. Install dsh first, then run this installer again.".to_owned(),
        );
    }
    let targets: Vec<_> = match profile_id.as_deref() {
        Some(id) => profiles
            .into_iter()
            .filter(|profile| profile.id == id)
            .collect(),
        None => profiles,
    };
    if targets.is_empty() {
        return Err(format!(
            "dsh profile {:?} was not found under the dsh profiles directory",
            profile_id
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        targets
            .into_iter()
            .map(|profile| install_dsh(&home, &profile, &package_dir))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|error| format!("Installer task failed: {error}"))?
}

// #endregion

#[cfg(test)]
#[path = "dsh_install/tests.rs"]
mod tests;
