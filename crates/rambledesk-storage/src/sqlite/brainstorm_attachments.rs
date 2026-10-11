//! Give inherited card attachments their own durable files and request identity.

use std::collections::{BTreeMap, HashSet};

use rambledesk_core::{BrainstormData, FeedbackPackageReader, IdGenerator, UuidV7Generator};
use tokio::io::AsyncWriteExt;

use super::*;

impl SqliteFeedbackStore {
    pub(super) async fn inherit_brainstorm_attachments(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        parent_request_id: &str,
        child_request_id: &str,
        now: &str,
        data: &mut BrainstormData,
    ) -> Result<Vec<PathBuf>, RepositoryError> {
        let parent = load_request_row(transaction, parent_request_id)
            .await?
            .ok_or(RepositoryError::RequestNotFound)?;
        let result = feedback_result_from_row(&parent)?.ok_or(RepositoryError::CorruptData)?;
        // The normal package reader validates identity, relative paths, sizes and hashes.
        let package = FeedbackPackageReader::read(self, parent_request_id, &result).await?;
        let mut known = HashSet::new();
        let mut referenced = Vec::new();
        for (attachment, source) in package
            .manifest
            .attachments
            .iter()
            .zip(&package.attachment_paths)
        {
            if !known.insert(attachment.id.clone()) {
                return Err(RepositoryError::CorruptData);
            }
            if data
                .initial_board
                .cards
                .iter()
                .any(|card| attachment_ids(&card.body).any(|id| id == attachment.id))
            {
                referenced.push((attachment, source));
            }
        }
        if referenced.is_empty() {
            return Ok(Vec::new());
        }
        let existing: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM attachments WHERE request_id = ?1")
                .bind(child_request_id)
                .fetch_one(&mut **transaction)
                .await
                .map_err(storage_error)?;
        if existing as usize + referenced.len() > MAX_ATTACHMENT_COUNT {
            return Err(RepositoryError::AttachmentLimit);
        }
        let directory = self
            .library_root()
            .join("drafts")
            .join(child_request_id)
            .join("attachments");
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(storage_error)?;
        let mut paths = Vec::new();
        let mut replacements = BTreeMap::new();
        let mutation = async {
            for (offset, (attachment, source)) in referenced.iter().enumerate() {
                let bytes = tokio::fs::read(source).await.map_err(storage_error)?;
                // Verify again at the copy boundary: a file may have changed after read().
                if bytes.len() as u64 != attachment.byte_size
                    || hex::encode(Sha256::digest(&bytes)) != attachment.sha256
                {
                    return Err(RepositoryError::PackageRead);
                }
                let id = UuidV7Generator.new_id();
                let path = directory.join(format!("{}-{}", id, portable_file_name(&attachment.file_name)));
                let mut file = tokio::fs::OpenOptions::new().write(true).create_new(true)
                    .open(&path).await.map_err(storage_error)?;
                // Record ownership only after create_new succeeds; never remove another file.
                paths.push(path.clone());
                file.write_all(&bytes).await.map_err(storage_error)?;
                file.flush().await.map_err(storage_error)?;
                file.sync_all().await.map_err(storage_error)?;
                sqlx::query(
                    "INSERT INTO attachments \
                     (id, request_id, draft_path, file_name, byte_size, media_type, sha256, position, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .bind(&id).bind(child_request_id).bind(path_string(&path)?)
                .bind(&attachment.file_name).bind(bytes.len() as i64).bind(&attachment.media_type)
                .bind(&attachment.sha256).bind(existing + offset as i64).bind(now)
                .execute(&mut **transaction).await.map_err(storage_error)?;
                replacements.insert(attachment.id.clone(), id);
            }
            Ok::<(), RepositoryError>(())
        }.await;
        if let Err(error) = mutation {
            for path in &paths {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Err(error);
        }
        for card in &mut data.initial_board.cards {
            card.body = rebind_attachment_ids(&card.body, &replacements);
        }
        Ok(paths)
    }
}

/// Internal attachment URIs are identities, not arbitrary substring prefixes.
fn attachment_ids(value: &str) -> impl Iterator<Item = &str> {
    value.match_indices("attachment://").map(|(start, _)| {
        let tail = &value[start + "attachment://".len()..];
        let end = tail
            .find(|character: char| {
                !character.is_ascii_alphanumeric() && !matches!(character, '-' | '_')
            })
            .unwrap_or(tail.len());
        &tail[..end]
    })
}

fn rebind_attachment_ids(value: &str, replacements: &BTreeMap<String, String>) -> String {
    let mut result = String::with_capacity(value.len());
    let mut cursor = 0;
    for (start, _) in value.match_indices("attachment://") {
        let id_start = start + "attachment://".len();
        let tail = &value[id_start..];
        let end = tail
            .find(|character: char| {
                !character.is_ascii_alphanumeric() && !matches!(character, '-' | '_')
            })
            .unwrap_or(tail.len());
        if let Some(replacement) = replacements.get(&tail[..end]) {
            result.push_str(&value[cursor..id_start]);
            result.push_str(replacement);
            cursor = id_start + end;
        }
    }
    result.push_str(&value[cursor..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebinds_repeated_exact_attachment_identities_without_changing_other_text() {
        let old = "019aa001-1000-7000-8000-000000000001";
        let new = "019aa001-2000-7000-8000-000000000002";
        let mapping = BTreeMap::from([(old.into(), new.into())]);
        let source = format!(
            "人改的😀文字 ![图](attachment://{old}) [材料](attachment://{old}#detail) attachment://{old}-other attachment://unknown"
        );
        let expected = format!(
            "人改的😀文字 ![图](attachment://{new}) [材料](attachment://{new}#detail) attachment://{old}-other attachment://unknown"
        );
        assert_eq!(rebind_attachment_ids(&source, &mapping), expected);
        assert_eq!(
            attachment_ids(&source).collect::<Vec<_>>(),
            vec![old, old, &format!("{old}-other"), "unknown"]
        );
        assert_eq!(rebind_attachment_ids("原文😀", &mapping), "原文😀");
    }
}
