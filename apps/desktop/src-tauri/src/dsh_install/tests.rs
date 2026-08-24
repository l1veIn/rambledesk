use std::fs;
use std::path::{Path, PathBuf};

use super::*;

/// Serializes the tests that mutate the process-wide `DSH_HOME` environment
/// variable, so parallel tests never observe another test's override.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn fake_repo(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rambledesk-{label}-{}", std::process::id()));
    let pkg = root.join("packages").join("dsh-rambledesk");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("package.json"), "{}").unwrap();
    fs::write(pkg.join("index.js"), "export const apply = () => {};\n").unwrap();
    root
}

fn fake_profile(root: &Path, id: &str, patch_content: &str) -> DshProfileView {
    let profile_dir = root.join("profiles").join(id);
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(profile_dir.join("cordis.patch.yml"), patch_content).unwrap();
    DshProfileView {
        id: id.to_owned(),
        profile_dir: profile_dir.to_string_lossy().into_owned(),
        patch_path: profile_dir
            .join("cordis.patch.yml")
            .to_string_lossy()
            .into_owned(),
        configured: patch_content.contains("id: rambledesk"),
    }
}

#[test]
fn package_dir_resolves_from_checkout_root() {
    let root = fake_repo("dsh-install-root");
    let expected = root.join("packages").join("dsh-rambledesk");
    assert_eq!(
        resolve_package_dir(Some(root.to_str().unwrap()), None),
        Some(expected.clone())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn package_dir_walks_up_from_nested_anchor() {
    let root = fake_repo("dsh-install-walk");
    let anchor = root.join("apps").join("desktop");
    fs::create_dir_all(&anchor).unwrap();
    assert_eq!(
        find_package_dir_from(&anchor),
        Some(root.join("packages").join("dsh-rambledesk"))
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn package_dir_missing_returns_none() {
    let root = std::env::temp_dir().join(format!("rambledesk-dsh-none-{}", std::process::id()));
    assert_eq!(find_package_dir_from(&root), None);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dsh_home_follows_dsh_home_override() {
    let _guard = ENV_LOCK.lock().unwrap();
    let home = std::env::temp_dir().join("rambledesk-dsh-home");
    let previous = std::env::var_os("DSH_HOME");
    unsafe {
        std::env::set_var("DSH_HOME", home.join("portable-dsh"));
    }
    assert_eq!(dsh_home(&home), home.join("portable-dsh"));
    match previous {
        Some(value) => unsafe { std::env::set_var("DSH_HOME", value) },
        None => unsafe { std::env::remove_var("DSH_HOME") },
    }
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn list_dsh_profiles_enumerates_only_patch_profiles() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = fake_repo("dsh-profiles");
    let previous = std::env::var_os("DSH_HOME");
    // Pin the override to the fixture so the enumeration never depends on the
    // ambient environment (a developer machine commonly has DSH_HOME set).
    unsafe {
        std::env::set_var("DSH_HOME", root.join(".dsh"));
    }
    let profile_dir = root.join(".dsh").join("profiles").join("web");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(profile_dir.join("cordis.patch.yml"), "[]\n").unwrap();
    // A directory without a patch file must not be reported as a profile.
    fs::create_dir_all(root.join(".dsh").join("profiles").join("bare")).unwrap();

    let profiles = list_dsh_profiles(&root);
    match previous {
        Some(value) => unsafe { std::env::set_var("DSH_HOME", value) },
        None => unsafe { std::env::remove_var("DSH_HOME") },
    }
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, "web");
    assert!(!profiles[0].configured);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn append_patch_entry_creates_the_insert_block() {
    let root = fake_repo("dsh-patch-create");
    let profile = fake_profile(&root, "web", "[]\n");

    let action = append_patch_entry(Path::new(&profile.patch_path)).unwrap();
    assert_eq!(action, "updated");
    let content = fs::read_to_string(&profile.patch_path).unwrap();
    assert!(content.contains("id: rambledesk"));
    assert!(content.contains("./plugins/rambledesk/index.js"));
    assert!(content.contains("hostId: dsh"));
    // The patch file is ONE top-level YAML array: the empty `[]` must be
    // replaced by the insert block, never followed by a second element.
    assert!(!content.contains("[]"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn append_patch_entry_replaces_an_empty_array_keeping_leading_comments() {
    let root = fake_repo("dsh-patch-comments");
    let profile = fake_profile(
        &root,
        "web",
        "# Your patch layer for this dsh profile.\n[]\n",
    );

    append_patch_entry(Path::new(&profile.patch_path)).unwrap();
    let content = fs::read_to_string(&profile.patch_path).unwrap();
    assert!(content.starts_with("# Your patch layer for this dsh profile.\n"));
    assert!(!content.contains("[]"));
    assert!(content.contains("- insert:"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn append_patch_entry_appends_to_a_nonempty_entry_list() {
    let root = fake_repo("dsh-patch-existing");
    let profile = fake_profile(&root, "web", "- id: other\n  name: '@scope/other-plugin'\n");

    let action = append_patch_entry(Path::new(&profile.patch_path)).unwrap();
    assert_eq!(action, "updated");
    let content = fs::read_to_string(&profile.patch_path).unwrap();
    assert!(content.starts_with("- id: other"));
    assert!(content.contains("- insert:"));
    assert!(content.contains("id: rambledesk"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn append_patch_entry_is_idempotent() {
    let root = fake_repo("dsh-patch-idempotent");
    let profile = fake_profile(&root, "web", "[]\n");
    append_patch_entry(Path::new(&profile.patch_path)).unwrap();
    let action = append_patch_entry(Path::new(&profile.patch_path)).unwrap();
    assert_eq!(action, "unchanged");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_dsh_copies_plugin_and_skill_idempotently() {
    let root = fake_repo("dsh-install");
    let package = root.join("packages").join("dsh-rambledesk");
    let profile = fake_profile(&root, "web", "[]\n");
    let home = root.join("home");

    let first = install_dsh(&home, &profile, &package).unwrap();
    assert_eq!(first.action, "updated");
    assert!(first.restart_required);
    assert!(
        Path::new(&profile.profile_dir)
            .join("plugins")
            .join("rambledesk")
            .join("index.js")
            .is_file()
    );
    let installed_skill = home
        .join(".agents")
        .join("skills")
        .join("ramble")
        .join("SKILL.md");
    assert_eq!(
        fs::read_to_string(installed_skill).unwrap(),
        RAMBLE_SKILL_MD
    );

    // Second run leaves everything byte-identical.
    let second = install_dsh(&home, &profile, &package).unwrap();
    assert_eq!(second.action, "unchanged");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_dsh_appends_the_entry_to_an_empty_profile() {
    let root = fake_repo("dsh-install-empty");
    let package = root.join("packages").join("dsh-rambledesk");
    let profile = DshProfileView {
        id: "web".to_owned(),
        profile_dir: root
            .join("profiles")
            .join("web")
            .to_string_lossy()
            .into_owned(),
        patch_path: root
            .join("profiles")
            .join("web")
            .join("cordis.patch.yml")
            .to_string_lossy()
            .into_owned(),
        configured: false,
    };
    fs::create_dir_all(root.join("profiles").join("web")).unwrap();
    fs::write(&profile.patch_path, "[]\n").unwrap();

    let result = install_dsh(&root.join("home"), &profile, &package).unwrap();
    assert_eq!(result.action, "updated");
    let content = fs::read_to_string(&profile.patch_path).unwrap();
    assert!(content.ends_with("- insert:\n    - id: rambledesk\n      name: './plugins/rambledesk/index.js'\n      config:\n        hostId: dsh\n"));
    let _ = fs::remove_dir_all(&root);
}
