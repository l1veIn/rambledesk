//! Runtime-only bridge between a verified pi-acp recipe and its managed extension.
use crate::{AcpLaunch, pi_wrapper};
use rambledesk_core::{AgentConfig, AgentDriverError, FeedbackTransport, ManagedFeedbackEndpoint};
use std::path::{Path, PathBuf};

// Deliberately not Debug: native command material and authorization stay private.
pub(crate) struct PreparedPiFeedback {
    companion: PathBuf,
    native: pi_wrapper::PiNativeLaunch,
    resource_root: PathBuf,
}

pub(crate) async fn select(
    config: &AgentConfig,
    http: bool,
    companion: Option<&Path>,
    resource_root: Option<&Path>,
) -> Result<(Option<FeedbackTransport>, Option<PreparedPiFeedback>), AgentDriverError> {
    if http || !pi_wrapper::is_pi_acp_recipe(&config.command, &config.args).await {
        return crate::feedback_transport::select(http, companion)
            .map(|transport| (transport, None));
    }
    // pi-acp 0.0.33 ignores mcpServers: never advertise the generic stdio route.
    let companion =
        crate::feedback_transport::validate_companion(companion.ok_or_else(unavailable)?)?;
    let resource_root = resource_root
        .filter(|root| root.is_absolute() && root.parent().is_some())
        .ok_or_else(unavailable)?
        .to_path_buf();
    let inherited = std::env::var("PI_ACP_PI_COMMAND").ok();
    let override_command = config
        .env
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("PI_ACP_PI_COMMAND"))
        .map(|(_, value)| value.as_str())
        .or(inherited.as_deref());
    let native =
        pi_wrapper::resolve_native_pi_for_agent(&config.command, &config.args, override_command)
            .await
            .map_err(|_| unavailable())?;
    if std::fs::canonicalize(&native.command).ok().as_ref() == Some(&companion) {
        return Err(unavailable());
    }
    Ok((
        Some(FeedbackTransport::PiExtension),
        Some(PreparedPiFeedback {
            companion,
            native,
            resource_root,
        }),
    ))
}

impl PreparedPiFeedback {
    pub(crate) async fn inject(
        self,
        options: &mut AcpLaunch,
        endpoint: ManagedFeedbackEndpoint,
    ) -> Result<(), AgentDriverError> {
        let extension = pi_wrapper::install_managed_extension(&self.resource_root)
            .await
            .map_err(|_| unavailable())?;
        let args = serde_json::to_string(&self.native.args).map_err(|_| unavailable())?;
        if args.len() > 64 * 1024 {
            return Err(unavailable());
        }
        options
            .env
            .retain(|key, _| !key.eq_ignore_ascii_case("PI_ACP_PI_COMMAND"));
        for (key, value) in [
            (
                "PI_ACP_PI_COMMAND",
                self.companion.to_string_lossy().into_owned(),
            ),
            (pi_wrapper::WRAPPER_ENV, "1".into()),
            (pi_wrapper::COMMAND_ENV, self.native.command),
            (pi_wrapper::ARGS_ENV, args),
            (
                pi_wrapper::EXTENSION_ENV,
                extension.to_string_lossy().into_owned(),
            ),
            (crate::feedback_transport::URL_ENV, endpoint.url),
            (crate::feedback_transport::TOKEN_ENV, endpoint.bearer_token),
        ] {
            options.env.insert(key.into(), value);
        }
        Ok(())
    }
}

fn unavailable() -> AgentDriverError {
    AgentDriverError::new(
        "Managed Pi feedback requires the RambleDesk wrapper, its runtime resource directory, and an available native Pi command",
    )
}
