use std::{net::IpAddr, path::Path};

use rambledesk_core::{AgentDriverError, ManagedFeedbackEndpoint};

/// Install only instance-local environment. The prompt contains no capability
/// URL or bearer, and the command resolves neither host nor session identity.
pub(crate) fn inject(
    options: &mut crate::AcpLaunch,
    endpoint: ManagedFeedbackEndpoint,
    companion: Option<&Path>,
) -> Result<String, AgentDriverError> {
    let invalid = || {
        AgentDriverError::new(
            "Managed feedback requires its local session capability and application command",
        )
    };
    let companion = crate::feedback_transport::validate_companion(companion.ok_or_else(invalid)?)?;
    let mut url = url::Url::parse(&endpoint.url).map_err(|_| invalid())?;
    let host = url.host_str().ok_or_else(invalid)?.trim_matches(['[', ']']);
    if url.scheme() != "http"
        || !host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "/mcp-managed" | "/agent-feedback")
        || url.port_or_known_default().is_none_or(|port| port == 0)
        || endpoint.bearer_token.len() != 64
        || !endpoint
            .bearer_token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid());
    }
    url.set_path("/agent-feedback");
    // Persisted values and inherited capabilities from a parent Agent cannot
    // choose the feedback scope, even on case-insensitive Windows environments.
    options.env = crate::feedback_transport::public_environment(&options.env);
    options.env.extend([
        ("RAMBLEDESK_MANAGED_SESSION".into(), "1".into()),
        (
            "RAMBLEDESK_COMMAND".into(),
            companion.to_string_lossy().into_owned(),
        ),
        ("RAMBLEDESK_FEEDBACK_URL".into(), url.into()),
        ("RAMBLEDESK_FEEDBACK_TOKEN".into(), endpoint.bearer_token),
    ]);
    Ok(include_str!("feedback_workflow.md").into())
}
