use sqlx::{Row, SqlitePool};

use crate::inspect::InspectError;

use super::read::{LegacyAction, LegacyContextRef, table_exists};

pub(super) const MAX_MIGRATION_ARTIFACT_BYTES: usize = 256 * 1024 * 1024;

pub(super) async fn load_actions(
    pool: &SqlitePool,
    request_id: &str,
) -> Result<Vec<LegacyAction>, InspectError> {
    if !table_exists(pool, "request_actions").await? {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT action_id, instruction FROM request_actions WHERE request_id = ?1 ORDER BY position",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(InspectError::SourceSchema)?;
    rows.into_iter()
        .map(|row| {
            Ok(LegacyAction {
                id: row
                    .try_get("action_id")
                    .map_err(InspectError::SourceSchema)?,
                instruction: row
                    .try_get("instruction")
                    .map_err(InspectError::SourceSchema)?,
            })
        })
        .collect()
}

pub(super) async fn load_context_refs(
    pool: &SqlitePool,
    request_id: &str,
) -> Result<Vec<LegacyContextRef>, InspectError> {
    if !table_exists(pool, "request_context_refs").await? {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT label, uri FROM request_context_refs WHERE request_id = ?1 ORDER BY position",
    )
    .bind(request_id)
    .fetch_all(pool)
    .await
    .map_err(InspectError::SourceSchema)?;
    rows.into_iter()
        .map(|row| {
            Ok(LegacyContextRef {
                label: row.try_get("label").map_err(InspectError::SourceSchema)?,
                uri: row.try_get("uri").map_err(InspectError::SourceSchema)?,
            })
        })
        .collect()
}

pub(super) fn nonblank(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_owned()
    } else {
        value
    }
}

pub(super) fn valid_action_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

pub(super) fn charge_artifact_bytes(
    total: &mut usize,
    additional: usize,
    legacy_id: &str,
) -> Result<(), InspectError> {
    *total = total.checked_add(additional).ok_or_else(|| {
        InspectError::ResourceLimit(format!("legacy object {legacy_id} byte count overflowed"))
    })?;
    if *total > MAX_MIGRATION_ARTIFACT_BYTES {
        return Err(InspectError::ResourceLimit(format!(
            "legacy object {legacy_id} exceeds the 256 MiB migration Artifact budget"
        )));
    }
    Ok(())
}
