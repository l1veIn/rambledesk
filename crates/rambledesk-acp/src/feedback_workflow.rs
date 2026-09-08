use std::{net::IpAddr, path::Path};

use rambledesk_core::{AgentDriverError, ManagedFeedbackEndpoint};
use rambledesk_feedback_client::channel::FeedbackChannel;

pub(crate) struct FeedbackWorkflow {
    pub channel: FeedbackChannel,
}

pub(crate) const INSTRUCTIONS: &str = include_str!("feedback_workflow.md");
pub(crate) const HANDOFF_REMINDER: &str = "RambleDesk did not receive a feedback handoff for this turn. Do not repeat the task or redo any work. Hand the result or question to the user now using the built-in feedback request command, then end the turn. First recover if a previous delivery was uncertain; never duplicate an existing request. Only if the user explicitly opted out, explicitly finished the task, or cancelled the request, record the matching feedback skip reason instead. A read-only answer, summary, or explanation still requires a Ramble handoff.";

/// Install only instance-local environment. The prompt contains no capability
/// URL or bearer, and the command resolves neither host nor session identity.
pub(crate) fn inject(
    options: &mut crate::AcpLaunch,
    endpoint: ManagedFeedbackEndpoint,
    companion: Option<&Path>,
) -> Result<FeedbackWorkflow, AgentDriverError> {
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
    let channel = FeedbackChannel::start(ManagedFeedbackEndpoint {
        url: url.into(),
        bearer_token: endpoint.bearer_token,
    })
    .map_err(|_| AgentDriverError::new("Could not create the private feedback channel"))?;
    // Persisted values and inherited capabilities from a parent Agent cannot
    // choose the feedback scope, even on case-insensitive Windows environments.
    options.env = crate::feedback_transport::public_environment(&options.env);
    options.env.extend([
        ("RAMBLEDESK_MANAGED_SESSION".into(), "1".into()),
        // Previously installed Pi adapters predate MANAGED_SESSION. Their entry
        // point recognizes this non-secret guard and skips external tools. This
        // is compatibility metadata, not an extension or a transport capability.
        ("RAMBLEDESK_MANAGED_PI_ACTIVE".into(), "1".into()),
        (
            "RAMBLEDESK_COMMAND".into(),
            companion.to_string_lossy().into_owned(),
        ),
        (
            "RAMBLEDESK_FEEDBACK_CHANNEL".into(),
            channel.address().into(),
        ),
    ]);
    Ok(FeedbackWorkflow { channel })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, time::Duration};
    use tokio::io::AsyncReadExt;

    async fn legacy_pi_registration(env: BTreeMap<String, String>) -> String {
        let directory = tempfile::tempdir().unwrap();
        let mut child = crate::process::spawn_filtered(
            "node",
            &[
                "--input-type=module".into(),
                "-e".into(),
                include_str!("../tests/fixtures/legacy_pi_registration.mjs").into(),
            ],
            &env,
            directory.path(),
            &crate::feedback_transport::inherited_private_env_to_remove(&env),
        )
        .unwrap();
        drop(child.take_stdin());
        let mut output = String::new();
        child
            .take_stdout()
            .unwrap()
            .read_to_string(&mut output)
            .await
            .unwrap();
        assert!(
            child
                .wait_with_timeout(Duration::from_secs(3))
                .await
                .unwrap()
                .unwrap()
                .success()
        );
        output.trim().into()
    }

    #[tokio::test]
    async fn managed_command_suppresses_previously_installed_pi_adapter_without_updating_it() {
        assert_eq!(
            legacy_pi_registration(BTreeMap::new()).await,
            "external tools registered"
        );
        let mut launch = crate::AcpLaunch {
            command: "node".into(),
            args: vec![],
            cwd: std::env::temp_dir(),
            env: BTreeMap::from([
                ("RAMBLEDESK_MANAGED_PI_ACTIVE".into(), "0".into()),
                (
                    "RAMBLEDESK_MANAGED_PI_EXTENSION".into(),
                    "/stale/extension.mjs".into(),
                ),
                (
                    "rambledesk_feedback_token".into(),
                    "stale-persistent-capability".into(),
                ),
                ("CUSTOM".into(), "preserved".into()),
            ]),
            mcp_servers: vec![],
        };
        let workflow = inject(
            &mut launch,
            ManagedFeedbackEndpoint {
                url: "http://127.0.0.1:37642/agent-feedback".into(),
                bearer_token: "a".repeat(64),
            },
            Some(&std::env::current_exe().unwrap()),
        )
        .unwrap();
        assert_eq!(launch.env["CUSTOM"], "preserved");
        assert!(!launch.env.contains_key("RAMBLEDESK_MANAGED_PI_EXTENSION"));
        assert!(!launch.env.contains_key("rambledesk_feedback_token"));
        assert!(!launch.env.contains_key("RAMBLEDESK_MANAGED_MCP_URL"));
        assert!(!launch.env.contains_key("RAMBLEDESK_MANAGED_MCP_TOKEN"));
        assert!(!INSTRUCTIONS.contains(&"a".repeat(64)));
        assert!(!launch.env.contains_key("RAMBLEDESK_FEEDBACK_TOKEN"));
        assert!(!launch.env.contains_key("RAMBLEDESK_FEEDBACK_URL"));
        assert_eq!(
            launch.env["RAMBLEDESK_FEEDBACK_CHANNEL"],
            workflow.channel.address()
        );
        assert_eq!(legacy_pi_registration(launch.env).await, "suppressed");
    }
}
