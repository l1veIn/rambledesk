use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;

pub async fn create_fixture(root: &Path) -> PathBuf {
    let database = root.join("feedback.sqlite3");
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .expect("create fixture database");
    sqlx::raw_sql(include_str!("../fixtures/mixed-v2.sql"))
        .execute(&pool)
        .await
        .expect("install mixed v2 fixture");

    let draft_library = root.join("draft-library");
    tokio::fs::create_dir_all(&draft_library)
        .await
        .expect("draft library");
    let draft_attachment = b"Draft screenshot bytes.\n";
    let draft_attachment_path = draft_library.join("draft-screenshot.png");
    tokio::fs::write(&draft_attachment_path, draft_attachment)
        .await
        .expect("draft attachment");
    sqlx::query(
        "INSERT INTO attachments \
         (id, request_id, draft_path, published_path, file_name, byte_size, media_type, sha256, position, created_at) \
         VALUES ('draft-attachment', 'request-waiting', ?1, NULL, 'draft-screenshot.png', ?2, \
                 'image/png', ?3, 0, '2026-08-01T00:20:00Z')",
    )
    .bind(draft_attachment_path.to_string_lossy().as_ref())
    .bind(draft_attachment.len() as i64)
    .bind(sha256_hex(draft_attachment))
    .execute(&pool)
    .await
    .expect("insert draft attachment");
    sqlx::query(
        "INSERT INTO attachments \
         (id, request_id, draft_path, published_path, file_name, byte_size, media_type, sha256, position, created_at) \
         VALUES ('draft-attachment-blank', 'request-waiting', ?1, NULL, '', ?2, \
                 '', ?3, 1, '2026-08-01T00:21:00Z')",
    )
    .bind(draft_attachment_path.to_string_lossy().as_ref())
    .bind(draft_attachment.len() as i64)
    .bind(sha256_hex(draft_attachment))
    .execute(&pool)
    .await
    .expect("insert draft attachment with blank metadata");

    let request_attachment = b"Request context attachment.\n";
    let request_attachment_path = draft_library.join("request-context.md");
    tokio::fs::write(&request_attachment_path, request_attachment)
        .await
        .expect("request attachment");
    insert_request_attachment(
        &pool,
        "request-attachment-waiting",
        "request-waiting",
        "request-context.md",
        "text/markdown",
        request_attachment,
        0,
        Some(&request_attachment_path),
        None,
    )
    .await;
    insert_request_attachment(
        &pool,
        "request-attachment-waiting-alias",
        "request-waiting",
        "request-context-copy.md",
        "text/markdown",
        request_attachment,
        1,
        Some(&request_attachment_path),
        None,
    )
    .await;

    let readable = root.join("packages").join("readable");
    tokio::fs::create_dir_all(&readable)
        .await
        .expect("readable package directory");
    let feedback = b"Structured human feedback.\n";
    let uncooked = b"Original ramble.\n";
    let attachment = b"Legacy screenshot bytes.\n";
    let completed_request_attachment = b"Completed request context.\n";
    tokio::fs::write(readable.join("feedback.md"), feedback)
        .await
        .expect("feedback");
    tokio::fs::write(readable.join("uncooked.md"), uncooked)
        .await
        .expect("uncooked");
    tokio::fs::create_dir_all(readable.join("attachments"))
        .await
        .expect("attachment directory");
    tokio::fs::write(
        readable.join("attachments").join("evidence.txt"),
        attachment,
    )
    .await
    .expect("attachment");
    tokio::fs::create_dir_all(readable.join("request-attachments"))
        .await
        .expect("request attachment directory");
    let published_request_attachment = readable.join("request-attachments").join("context.md");
    tokio::fs::write(&published_request_attachment, completed_request_attachment)
        .await
        .expect("completed request attachment");
    let manifest = serde_json::to_string_pretty(&json!({
        "schema_version": 1,
        "request_id": "request-completed-readable",
        "feedback_markdown": "feedback.md",
        "feedback_sha256": sha256_hex(feedback),
        "uncooked_markdown": "uncooked.md",
        "uncooked_sha256": sha256_hex(uncooked),
        "attachments": [{
            "path": "attachments/evidence.txt",
            "byte_size": attachment.len(),
            "sha256": sha256_hex(attachment)
        }],
        "request_attachments": [{
            "path": "request-attachments/context.md",
            "byte_size": completed_request_attachment.len(),
            "sha256": sha256_hex(completed_request_attachment)
        }]
    }))
    .expect("manifest json")
        + "\n";
    tokio::fs::write(readable.join("manifest.json"), manifest.as_bytes())
        .await
        .expect("manifest");
    insert_result(
        &pool,
        "request-completed-readable",
        &readable,
        &sha256_hex(manifest.as_bytes()),
    )
    .await;
    insert_request_attachment(
        &pool,
        "request-attachment-completed",
        "request-completed-readable",
        "context.md",
        "text/markdown",
        completed_request_attachment,
        0,
        None,
        Some(&published_request_attachment),
    )
    .await;

    let missing = root.join("packages").join("missing");
    insert_result(
        &pool,
        "request-completed-unreadable",
        &missing,
        &"0".repeat(64),
    )
    .await;
    pool.close().await;
    database
}

#[allow(dead_code)]
pub async fn add_unsafe_parent_package(root: &Path, database: &Path) -> Vec<u8> {
    let secret = b"TOP SECRET: migration must not copy this file.\n".to_vec();
    tokio::fs::write(root.join("unrelated-secret.txt"), &secret)
        .await
        .expect("unrelated secret");
    let feedback = b"Package placed in an unsafe parent.\n";
    tokio::fs::write(root.join("feedback.md"), feedback)
        .await
        .expect("unsafe parent feedback");
    let manifest = serde_json::to_string_pretty(&json!({
        "schema_version": 1,
        "request_id": "request-completed-unsafe-parent",
        "feedback_markdown": "feedback.md",
        "feedback_sha256": sha256_hex(feedback),
        "attachments": []
    }))
    .expect("unsafe parent manifest json")
        + "\n";
    tokio::fs::write(root.join("manifest.json"), manifest.as_bytes())
        .await
        .expect("unsafe parent manifest");

    let options = SqliteConnectOptions::new()
        .filename(database)
        .create_if_missing(false);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .expect("open fixture for unsafe package");
    sqlx::query(
        "INSERT INTO feedback_requests \
         (id, host_session_record_id, title, what_happened, status, revision, input_hash, \
          created_at, updated_at, completed_at, allow_finish, resolution) \
         VALUES ('request-completed-unsafe-parent', 'session-1', 'Unsafe parent', \
                 'Package directory is the database parent', 'completed', 1, 'hash', \
                 '2026-08-01T09:00:00Z', '2026-08-01T09:00:00Z', \
                 '2026-08-01T09:00:00Z', 0, 'feedback_submitted')",
    )
    .execute(&pool)
    .await
    .expect("insert unsafe request");
    insert_result(
        &pool,
        "request-completed-unsafe-parent",
        root,
        &sha256_hex(manifest.as_bytes()),
    )
    .await;
    pool.close().await;
    secret
}

#[allow(clippy::too_many_arguments)]
async fn insert_request_attachment(
    pool: &sqlx::SqlitePool,
    id: &str,
    request_id: &str,
    file_name: &str,
    media_type: &str,
    contents: &[u8],
    position: i64,
    draft_path: Option<&Path>,
    published_path: Option<&Path>,
) {
    sqlx::query(
        "INSERT INTO request_attachments \
         (id, request_id, file_name, byte_size, media_type, sha256, position, contents, created_at, draft_path, published_path) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, x'', '2026-08-01T00:10:00Z', ?8, ?9)",
    )
    .bind(id)
    .bind(request_id)
    .bind(file_name)
    .bind(contents.len() as i64)
    .bind(media_type)
    .bind(sha256_hex(contents))
    .bind(position)
    .bind(draft_path.map(|path| path.to_string_lossy().into_owned()))
    .bind(published_path.map(|path| path.to_string_lossy().into_owned()))
    .execute(pool)
    .await
    .expect("insert request attachment");
}

async fn insert_result(
    pool: &sqlx::SqlitePool,
    request_id: &str,
    directory: &Path,
    manifest_sha256: &str,
) {
    sqlx::query(
        "INSERT INTO feedback_results \
         (request_id, package_uri, directory_path, markdown_path, manifest_path, manifest_sha256, published_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '2026-08-01T08:00:00Z')",
    )
    .bind(request_id)
    .bind(format!("rambledesk://feedback/{request_id}"))
    .bind(directory.to_string_lossy().as_ref())
    .bind(directory.join("feedback.md").to_string_lossy().as_ref())
    .bind(directory.join("manifest.json").to_string_lossy().as_ref())
    .bind(manifest_sha256)
    .execute(pool)
    .await
    .expect("insert feedback result");
}

pub fn sha256_hex(contents: &[u8]) -> String {
    hex::encode(Sha256::digest(contents))
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSnapshot {
    pub bytes: Vec<u8>,
    pub modified: SystemTime,
}

#[allow(dead_code)]
pub fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, FileSnapshot> {
    let mut snapshot = BTreeMap::new();
    collect_files(root, root, &mut snapshot);
    snapshot
}

#[allow(dead_code)]
fn collect_files(root: &Path, current: &Path, snapshot: &mut BTreeMap<PathBuf, FileSnapshot>) {
    let mut entries = std::fs::read_dir(current)
        .expect("read snapshot directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect snapshot directory");
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("snapshot metadata for {}: {error}", path.display()),
        };
        assert!(
            !metadata.file_type().is_symlink(),
            "fixture contains symlink"
        );
        if metadata.is_dir() {
            collect_files(root, &path, snapshot);
        } else if metadata.is_file() {
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => panic!("snapshot contents for {}: {error}", path.display()),
            };
            snapshot.insert(
                path.strip_prefix(root)
                    .expect("snapshot relative path")
                    .to_owned(),
                FileSnapshot {
                    bytes,
                    modified: metadata.modified().expect("snapshot modified time"),
                },
            );
        }
    }
}
