//! One-click installation of the dsh-native RambleDesk plugin.
//!
//! The dsh adapter is a Cordis plugin (`packages/dsh-rambledesk`) that talks to
//! the authenticated loopback JSON API directly and waits inside the dsh tool
//! call, so no post-submit continuation strategy is needed. Installing it is:
//!
//! 1. copy the plugin package next to a dsh profile
//!    (`<profile>/plugins/rambledesk`),
//! 2. append the loader entry to the profile's `cordis.patch.yml` (idempotent),
//! 3. install the dsh-customized `ramble` skill into the user's global skill
//!    directory (`~/.agents/skills/ramble/SKILL.md`), which dsh's
//!    skill-filesystem provider discovers.
//!
//! The patch file is a top-level YAML array; row order carries no load
//! semantics, so appending a fresh `- insert:` block is always safe. dsh
//! resolves relative plugin specifiers against the profile directory, so the
//! plugin is addressed as `./plugins/rambledesk/index.js`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use rambledesk_core::find_executable;

const PATCH_ID: &str = "rambledesk";
const PLUGIN_SUBDIR: &str = "plugins/rambledesk";
const SKILL_SOURCE_RELATIVE: &str = "skills/ramble/SKILL.md";
const SKILL_TARGET_RELATIVE: &str = ".agents/skills/ramble/SKILL.md";
const PATCH_ENTRY: &str = "\n- insert:\n    - id: rambledesk\n      name: './plugins/rambledesk/index.js'\n      config:\n        hostId: dsh\n";

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
pub fn detect_dsh_host(home: &Path) -> DshHostView {
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

    let skill_action = install_ramble_skill(package_dir, home)?;

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
            if file_name == "skills" {
                let source_skill = source.join("ramble").join("SKILL.md");
                let target_skill = target.join("skills").join("ramble").join("SKILL.md");
                if source_skill.is_file() {
                    copied |= copy_if_changed(&source_skill, &target_skill)?;
                }
            }
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
    if content.ends_with('\n') || content.is_empty() {
        fs::write(patch_path, format!("{content}{PATCH_ENTRY}"))
            .map_err(|error| format!("Could not write {}: {error}", patch_path.display()))?;
    } else {
        fs::write(patch_path, format!("{content}\n{PATCH_ENTRY}"))
            .map_err(|error| format!("Could not write {}: {error}", patch_path.display()))?;
    }
    Ok(if existed { "updated" } else { "created" })
}

/// Install the dsh-customized `ramble` skill into `~/.agents/skills`.
fn install_ramble_skill(package_dir: &Path, home: &Path) -> Result<&'static str, String> {
    let source = package_dir.join(SKILL_SOURCE_RELATIVE);
    if !source.is_file() {
        return Ok("unchanged");
    }
    let target = home.join(SKILL_TARGET_RELATIVE);
    Ok(if copy_if_changed(&source, &target)? {
        "updated"
    } else {
        "unchanged"
    })
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

#[cfg(test)]
#[path = "dsh_install/tests.rs"]
mod tests;
