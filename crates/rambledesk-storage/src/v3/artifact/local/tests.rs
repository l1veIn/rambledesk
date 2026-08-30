use std::path::Path;

use rambledesk_core::kernel::ports::{ArtifactStore, ArtifactStoreError, PutArtifact};
use tempfile::TempDir;

use super::{LocalArtifactStore, sha256};

async fn test_store() -> (TempDir, LocalArtifactStore) {
    let root = tempfile::tempdir().expect("temporary library root");
    let store = LocalArtifactStore::open(root.path())
        .await
        .expect("open local Artifact Store");
    (root, store)
}

fn input(contents: &[u8]) -> PutArtifact {
    PutArtifact {
        contents: contents.to_vec(),
        expected_sha256: sha256(contents),
    }
}

#[tokio::test]
async fn identical_bytes_are_idempotent() {
    let (_root, store) = test_store().await;
    let first = store
        .put(input(b"same immutable bytes"))
        .await
        .expect("put");
    let second = store
        .put(input(b"same immutable bytes"))
        .await
        .expect("idempotent put");

    assert_eq!(first, second);
    let shard = store
        .sha256_root
        .join(first.storage_key.split('/').nth(1).expect("digest shard"));
    assert_eq!(
        std::fs::read_dir(shard)
            .expect("read shard")
            .filter_map(Result::ok)
            .count(),
        1
    );
}

#[tokio::test]
async fn independent_stores_publish_the_same_digest_without_clobbering() {
    let root = tempfile::tempdir().expect("temporary library root");
    let first_store = LocalArtifactStore::open(root.path())
        .await
        .expect("open first store");
    let second_store = LocalArtifactStore::open(root.path())
        .await
        .expect("open second store");

    let (first, second) = tokio::join!(
        first_store.put(input(b"cross-instance immutable bytes")),
        second_store.put(input(b"cross-instance immutable bytes")),
    );
    let first = first.expect("first publisher");
    let second = second.expect("second publisher");

    assert_eq!(first, second);
    assert_eq!(
        first_store
            .open_verified(&first.storage_key, &first.sha256)
            .await
            .expect("verified winner"),
        b"cross-instance immutable bytes"
    );
    assert_eq!(
        std::fs::read_dir(first_store.staging_root.as_ref())
            .expect("read staging")
            .filter_map(Result::ok)
            .count(),
        0
    );
}

#[tokio::test]
async fn expected_digest_mismatch_does_not_publish() {
    let (_root, store) = test_store().await;
    let error = store
        .put(PutArtifact {
            contents: b"actual".to_vec(),
            expected_sha256: sha256(b"different"),
        })
        .await
        .expect_err("digest mismatch");

    assert_eq!(error, ArtifactStoreError::DigestMismatch);
    assert_eq!(
        std::fs::read_dir(store.staging_root.as_ref())
            .expect("read staging")
            .filter_map(Result::ok)
            .count(),
        0
    );
}

#[tokio::test]
async fn corrupt_existing_blob_is_never_overwritten() {
    let (_root, store) = test_store().await;
    let stored = store.put(input(b"expected bytes")).await.expect("put");
    let path = store.blob_path(&stored.storage_key).expect("blob path");
    tokio::fs::write(&path, b"corrupt bytes")
        .await
        .expect("corrupt stored blob");

    let error = store
        .put(input(b"expected bytes"))
        .await
        .expect_err("corrupt destination must fail");
    assert_eq!(error, ArtifactStoreError::DigestMismatch);
    assert_eq!(
        tokio::fs::read(path).await.expect("corrupt blob preserved"),
        b"corrupt bytes"
    );
}

#[tokio::test]
async fn no_clobber_publish_preserves_a_preexisting_winner() {
    let (_root, store) = test_store().await;
    let expected_bytes = b"staged expected bytes";
    let expected_sha256 = sha256(expected_bytes);
    let hex = expected_sha256
        .strip_prefix("sha256:")
        .expect("digest prefix");
    let shard = store.prepare_shard(&hex[..2]).await.expect("prepare shard");
    let final_path = shard.join(&hex[2..]);
    let staging_path = store.staging_path();

    tokio::fs::write(&staging_path, expected_bytes)
        .await
        .expect("write staged bytes");
    tokio::fs::write(&final_path, b"preexisting winner")
        .await
        .expect("write winner");

    let error = store
        .publish_staged(&staging_path, &final_path, &shard, &expected_sha256)
        .await
        .expect_err("different winner must be rejected");

    assert_eq!(error, ArtifactStoreError::DigestMismatch);
    assert_eq!(
        tokio::fs::read(&final_path)
            .await
            .expect("winner preserved"),
        b"preexisting winner"
    );
    assert_eq!(
        tokio::fs::symlink_metadata(&staging_path)
            .await
            .expect_err("staging cleaned")
            .kind(),
        std::io::ErrorKind::NotFound
    );
}

#[tokio::test]
async fn open_returns_only_verified_bytes() {
    let (_root, store) = test_store().await;
    let stored = store.put(input(b"verified artifact")).await.expect("put");

    let contents = store
        .open_verified(&stored.storage_key, &stored.sha256)
        .await
        .expect("verified read");
    assert_eq!(contents, b"verified artifact");

    let mismatch = store
        .open_verified(&stored.storage_key, &sha256(b"other"))
        .await
        .expect_err("expected hash must match key");
    assert_eq!(mismatch, ArtifactStoreError::DigestMismatch);
}

#[tokio::test]
async fn storage_key_never_exposes_an_absolute_path() {
    let (root, store) = test_store().await;
    let stored = store.put(input(b"opaque key")).await.expect("put");

    assert!(!Path::new(&stored.storage_key).is_absolute());
    assert!(
        !stored
            .storage_key
            .contains(root.path().to_string_lossy().as_ref())
    );
    assert!(stored.storage_key.starts_with("sha256/"));
}

#[tokio::test]
async fn absolute_and_traversal_keys_are_rejected() {
    let (_root, store) = test_store().await;
    let digest = sha256(b"key validation");

    assert_eq!(
        store.open_verified("/tmp/blob", &digest).await,
        Err(ArtifactStoreError::Storage)
    );
    assert_eq!(
        store.open_verified("sha256/../outside", &digest).await,
        Err(ArtifactStoreError::Storage)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlinked_digest_shard_is_rejected_without_writing_outside() {
    use std::os::unix::fs::symlink;

    let (root, store) = test_store().await;
    let artifact = input(b"symlink escape");
    let hex = artifact
        .expected_sha256
        .strip_prefix("sha256:")
        .expect("digest prefix");
    let shard = store.sha256_root.join(&hex[..2]);
    let outside = root.path().join("outside");
    tokio::fs::create_dir(&outside)
        .await
        .expect("outside directory");
    symlink(&outside, &shard).expect("symlink shard");

    let error = store.put(artifact).await.expect_err("reject symlink shard");
    assert_eq!(error, ArtifactStoreError::Storage);
    assert_eq!(
        std::fs::read_dir(outside)
            .expect("outside remains readable")
            .count(),
        0
    );
}
