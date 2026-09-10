use std::collections::BTreeMap;

use super::*;

impl SqliteFeedbackStore {
    pub(super) async fn save_agent_config_impl(
        &self,
        config: AgentConfig,
    ) -> Result<AgentConfig, SessionRepositoryError> {
        if !nonempty(&config.id)
            || config.catalog_id.as_ref().is_some_and(|id| !nonempty(id))
            || !nonempty(&config.name)
            || !nonempty(&config.host_id)
            || !nonempty(&config.command)
            || !nonempty(&config.created_at)
            || !nonempty(&config.updated_at)
            || config.args.iter().any(|value| value.contains('\0'))
            || config
                .env
                .iter()
                .any(|(key, value)| !nonempty(key) || key.contains('=') || value.contains('\0'))
        {
            return Err(SessionRepositoryError::InvalidInput);
        }
        let args_json = serde_json::to_string(&config.args).map_err(storage_error)?;
        let env_json = serde_json::to_string(&config.env).map_err(storage_error)?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let conflict: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM agent_configs ac \
             WHERE ac.id = ?1 AND ac.host_id != ?2 AND EXISTS \
             (SELECT 1 FROM managed_sessions ms WHERE ms.agent_config_id = ac.id))",
        )
        .bind(&config.id)
        .bind(&config.host_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if conflict {
            return Err(SessionRepositoryError::AgentConfigInUse);
        }
        sqlx::query(
            "INSERT INTO agent_configs \
             (id, name, host_id, protocol, enabled, command, args_json, env_json, created_at, updated_at, catalog_id) \
             VALUES (?1, ?2, ?3, 'acp', ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, host_id = excluded.host_id, \
             enabled = excluded.enabled, command = excluded.command, args_json = excluded.args_json, \
             env_json = excluded.env_json, updated_at = excluded.updated_at, catalog_id = excluded.catalog_id",
        )
        .bind(&config.id)
        .bind(&config.name)
        .bind(&config.host_id)
        .bind(config.enabled)
        .bind(&config.command)
        .bind(args_json)
        .bind(env_json)
        .bind(&config.created_at)
        .bind(&config.updated_at)
        .bind(&config.catalog_id)
        .execute(&mut *transaction)
        .await
        .map_err(write_error)?;
        transaction.commit().await.map_err(storage_error)?;
        self.get_agent_config_impl(&config.id).await
    }

    pub(super) async fn get_agent_config_impl(
        &self,
        id: &str,
    ) -> Result<AgentConfig, SessionRepositoryError> {
        let row = sqlx::query("SELECT * FROM agent_configs WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?
            .ok_or(SessionRepositoryError::AgentConfigNotFound)?;
        config_from_row(&row)
    }

    pub(super) async fn list_agent_configs_impl(
        &self,
    ) -> Result<Vec<AgentConfig>, SessionRepositoryError> {
        let rows = sqlx::query("SELECT * FROM agent_configs ORDER BY name, id")
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?;
        rows.iter().map(config_from_row).collect()
    }

    pub(super) async fn delete_agent_config_impl(
        &self,
        id: &str,
    ) -> Result<(), SessionRepositoryError> {
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let in_use: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE agent_config_id = ?1)",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if in_use {
            return Err(SessionRepositoryError::AgentConfigInUse);
        }
        let deleted = sqlx::query("DELETE FROM agent_configs WHERE id = ?1")
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        if deleted.rows_affected() == 0 {
            return Err(SessionRepositoryError::AgentConfigNotFound);
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }
}

fn config_from_row(row: &SqliteRow) -> Result<AgentConfig, SessionRepositoryError> {
    let protocol: String = row.try_get("protocol").map_err(storage_error)?;
    if protocol != "acp" {
        return Err(SessionRepositoryError::CorruptData);
    }
    let args: Vec<String> = serde_json::from_str(
        &row.try_get::<String, _>("args_json")
            .map_err(storage_error)?,
    )
    .map_err(|_| SessionRepositoryError::CorruptData)?;
    let env: BTreeMap<String, String> = serde_json::from_str(
        &row.try_get::<String, _>("env_json")
            .map_err(storage_error)?,
    )
    .map_err(|_| SessionRepositoryError::CorruptData)?;
    Ok(AgentConfig {
        id: row.try_get("id").map_err(storage_error)?,
        catalog_id: row.try_get("catalog_id").map_err(storage_error)?,
        name: row.try_get("name").map_err(storage_error)?,
        host_id: row.try_get("host_id").map_err(storage_error)?,
        protocol: SessionProtocol::Acp,
        enabled: row.try_get("enabled").map_err(storage_error)?,
        command: row.try_get("command").map_err(storage_error)?,
        args,
        env,
        created_at: row.try_get("created_at").map_err(storage_error)?,
        updated_at: row.try_get("updated_at").map_err(storage_error)?,
    })
}
