use crate::{
    AgentConfig, AgentDriverError, AgentManagementApplication, AgentVerificationStatus,
    CatalogAgentInput, ResolveCatalogAgentInput, SaveAgentConfigInput, SessionApplication,
    SessionProtocol,
};

impl AgentManagementApplication {
    /// Resolve an explicit selection without changing existing launch settings or starting an Agent.
    pub async fn resolve_configuration(
        &self,
        sessions: &SessionApplication,
        input: ResolveCatalogAgentInput,
    ) -> Result<AgentConfig, AgentDriverError> {
        let _guard = self.catalog_resolution.lock().await;
        let fail = |message: &str| AgentDriverError::new(message.to_owned());
        let entry = self
            .catalog()
            .into_iter()
            .find(|entry| entry.id == input.agent_id)
            .ok_or_else(|| fail("Unknown Agent catalog entry"))?;
        let profiles = sessions
            .list_agent_configs()
            .await
            .map_err(|error| fail(&error.to_string()))?
            .into_iter()
            .filter(|config| config.catalog_id.as_deref() == Some(&input.agent_id))
            .collect::<Vec<_>>();
        let existing = if let Some(id) = input.agent_config_id {
            Some(
                profiles
                    .into_iter()
                    .find(|config| config.id == id)
                    .ok_or_else(|| {
                        fail("The selected configuration does not belong to this Agent")
                    })?,
            )
        } else {
            if profiles.len() > 1 {
                return Err(fail("Choose a specific configuration for this Agent"));
            }
            profiles.into_iter().next()
        };
        if let Some(config) = existing {
            if config.enabled {
                return Ok(config);
            }
            if !input.enable {
                return Err(fail(
                    "This Agent configuration is disabled. Enable it explicitly before starting a session.",
                ));
            }
            return sessions
                .save_agent_config(SaveAgentConfigInput {
                    id: Some(config.id),
                    catalog_id: config.catalog_id,
                    name: config.name,
                    host_id: config.host_id,
                    protocol: config.protocol,
                    enabled: true,
                    command: config.command,
                    args: config.args,
                    env: config.env,
                })
                .await
                .map_err(|error| fail(&error.to_string()));
        }
        if entry.verification.status == AgentVerificationStatus::Unsupported {
            return Err(fail(
                "This Agent does not provide the feedback capability required by RambleDesk",
            ));
        }
        let inspection = self
            .inspect(CatalogAgentInput {
                agent_id: input.agent_id.clone(),
            })
            .await?;
        if inspection
            .checks
            .iter()
            .any(|check| check.status == crate::AgentCheckStatus::Fail)
        {
            return Err(fail(
                "Resolve the failed installation checks before starting this Agent",
            ));
        }
        let command = inspection
            .command
            .ok_or_else(|| fail("Install this Agent before starting a session"))?;
        sessions
            .save_agent_config(SaveAgentConfigInput {
                id: None,
                catalog_id: Some(input.agent_id),
                name: entry.name,
                host_id: entry.host_id,
                protocol: SessionProtocol::Acp,
                enabled: true,
                command,
                args: inspection.args,
                env: inspection.env.unwrap_or_default(),
            })
            .await
            .map_err(|error| fail(&error.to_string()))
    }
}
