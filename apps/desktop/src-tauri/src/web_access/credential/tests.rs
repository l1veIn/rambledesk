use std::{
    fs,
    os::unix::fs::{FileTypeExt, PermissionsExt, symlink},
    path::Path,
    process::Command,
    sync::{Arc, Barrier, mpsc},
    thread,
    time::Duration,
};

use super::FileWebAccessCredentialStore;
use crate::web_access::WebAccessCredentialStore;

fn mode(path: &Path) -> u32 {
    fs::metadata(path)
        .expect("test path metadata")
        .permissions()
        .mode()
        & 0o777
}

#[test]
fn reopening_keeps_the_token_and_rotation_persists_the_replacement() {
    let root = tempfile::tempdir().expect("temporary credential root");
    let directory = root.path().join("auth");
    let store = FileWebAccessCredentialStore::new(directory.clone());
    let original = store.load_or_create().expect("create credential");

    assert_eq!(mode(&directory), 0o700);
    assert_eq!(mode(&directory.join("web-access.token")), 0o600);
    assert_eq!(
        FileWebAccessCredentialStore::new(directory.clone())
            .load_or_create()
            .expect("reopen credential"),
        original,
    );

    let replacement = store.rotate().expect("rotate credential");
    assert_ne!(replacement, original);
    assert_eq!(
        FileWebAccessCredentialStore::new(directory.clone())
            .load_or_create()
            .expect("reopen rotated credential"),
        replacement,
    );
    assert_eq!(mode(&directory.join("web-access.token")), 0o600);
    assert_eq!(
        fs::read_dir(directory)
            .expect("credential directory")
            .count(),
        1
    );
}

#[test]
fn malformed_credentials_fail_without_replacing_or_exposing_their_contents() {
    let root = tempfile::tempdir().expect("temporary credential root");
    let directory = root.path().join("auth");
    fs::create_dir(&directory).expect("credential directory");
    let path = directory.join("web-access.token");
    let malformed = "malformed-private-credential-that-must-not-enter-an-error";
    fs::write(&path, malformed).expect("malformed credential fixture");

    let store = FileWebAccessCredentialStore::new(directory);
    let error = store
        .load_or_create()
        .expect_err("malformed credentials must require explicit recovery");

    assert!(
        !error.contains(malformed),
        "credential contents leaked in error"
    );
    assert_eq!(
        fs::read_to_string(path).expect("unchanged fixture"),
        malformed
    );

    let replacement = store.rotate().expect("explicit credential recovery");
    assert_eq!(
        store.load_or_create().expect("reopen recovered credential"),
        replacement,
    );
}

#[test]
fn existing_user_owned_permissions_are_tightened_without_changing_the_token() {
    let root = tempfile::tempdir().expect("temporary credential root");
    let directory = root.path().join("auth");
    let store = FileWebAccessCredentialStore::new(directory.clone());
    let token = store.load_or_create().expect("create credential");
    let path = directory.join("web-access.token");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
        .expect("broaden fixture directory permissions");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
        .expect("broaden fixture token permissions");

    assert_eq!(store.load_or_create().expect("repair permissions"), token);
    assert_eq!(mode(&directory), 0o700);
    assert_eq!(mode(&path), 0o600);
}

#[test]
fn symlinked_directory_or_token_is_rejected_without_touching_the_target() {
    let root = tempfile::tempdir().expect("temporary credential root");
    let target_directory = root.path().join("unrelated");
    fs::create_dir(&target_directory).expect("unrelated fixture directory");
    fs::set_permissions(&target_directory, fs::Permissions::from_mode(0o755))
        .expect("target directory permissions");
    let target = target_directory.join("web-access.token");
    let contents = "a".repeat(64);
    fs::write(&target, &contents).expect("unrelated fixture token");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
        .expect("target file permissions");

    let linked_directory = root.path().join("linked-auth");
    symlink(&target_directory, &linked_directory).expect("fixture directory symlink");
    let direct_directory = root.path().join("auth");
    fs::create_dir(&direct_directory).expect("credential directory");
    symlink(&target, direct_directory.join("web-access.token")).expect("fixture token symlink");

    for directory in [linked_directory, direct_directory] {
        let store = FileWebAccessCredentialStore::new(directory);
        assert!(
            store.load_or_create().is_err(),
            "must reject linked credential"
        );
        assert!(store.rotate().is_err(), "must not rotate through a symlink");
    }
    assert_eq!(
        fs::read_to_string(&target).expect("unchanged target"),
        contents
    );
    assert_eq!(mode(&target_directory), 0o755);
    assert_eq!(mode(&target), 0o644);
}

#[test]
fn directories_and_fifos_cannot_be_loaded_or_replaced_as_credentials() {
    let root = tempfile::tempdir().expect("temporary credential root");
    let directory = root.path().join("directory-auth");
    let token_directory = directory.join("web-access.token");
    fs::create_dir_all(&token_directory).expect("directory token fixture");
    let store = FileWebAccessCredentialStore::new(directory);
    assert!(store.load_or_create().is_err());
    assert!(store.rotate().is_err());
    assert!(token_directory.is_dir());

    let directory = root.path().join("fifo-auth");
    fs::create_dir(&directory).expect("FIFO credential directory");
    let fifo = directory.join("web-access.token");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("create test-owned FIFO")
            .success(),
    );
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let store = FileWebAccessCredentialStore::new(directory);
        let result = (store.load_or_create().is_err(), store.rotate().is_err());
        let _ = sender.send(result);
    });
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("reject a FIFO without waiting for a writer"),
        (true, true),
    );
    assert!(
        fs::symlink_metadata(fifo)
            .expect("unchanged FIFO")
            .file_type()
            .is_fifo()
    );
}

#[test]
fn concurrent_first_loads_obtain_one_complete_token_and_leave_no_staging_files() {
    let root = tempfile::tempdir().expect("temporary credential root");
    let directory = root.path().join("auth");
    let barrier = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let barrier = barrier.clone();
            let directory = directory.clone();
            thread::spawn(move || {
                let store = FileWebAccessCredentialStore::new(directory);
                barrier.wait();
                store
                    .load_or_create()
                    .expect("concurrent credential creation")
            })
        })
        .collect();
    let tokens: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("credential worker"))
        .collect();

    assert!(tokens.iter().all(|token| token == &tokens[0]));
    assert_eq!(
        FileWebAccessCredentialStore::new(directory.clone())
            .load_or_create()
            .expect("reopen shared credential"),
        tokens[0],
    );
    let names: Vec<_> = fs::read_dir(directory)
        .expect("credential directory")
        .map(|entry| entry.expect("credential directory entry").file_name())
        .collect();
    assert_eq!(names, vec!["web-access.token"]);
}

#[test]
fn separate_application_directories_have_independent_tokens_and_rotation() {
    let root = tempfile::tempdir().expect("temporary application roots");
    let first_directory = root.path().join("app-one").join("auth");
    let second_directory = root.path().join("app-two").join("auth");
    let first = FileWebAccessCredentialStore::new(first_directory);
    let second = FileWebAccessCredentialStore::new(second_directory);
    let first_token = first
        .load_or_create()
        .expect("first application credential");
    let second_token = second
        .load_or_create()
        .expect("second application credential");
    assert_ne!(first_token, second_token);

    assert_ne!(
        first.rotate().expect("rotate first application"),
        first_token
    );
    assert_eq!(
        second
            .load_or_create()
            .expect("unchanged second application"),
        second_token,
    );
}
