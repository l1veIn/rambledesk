//! Launch and initialize only. Never creates an Agent session or sends a prompt.
use crate::AcpConnection;
use rambledesk_core::{
    AgentConfig, AgentConnectionCheck, AgentFailure, AgentFailureReason as Reason,
    AgentFailureStage as Stage, FeedbackTransport, agent_operation_trace::AgentOperationTrace,
};
use std::{path::Path, sync::Arc};

pub(crate) async fn check(config: &AgentConfig, companion: Option<&Path>) -> AgentConnectionCheck {
    let mut trace = AgentOperationTrace::new("acp.check", Some(&config.id));
    let result = check_inner(config, companion, &trace).await;
    trace.finish(
        if result.ok { "succeeded" } else { "failed" },
        if result.failure.is_some() {
            "agent_check"
        } else {
            ""
        },
    );
    result
}

async fn check_inner(
    config: &AgentConfig,
    companion: Option<&Path>,
    trace: &AgentOperationTrace,
) -> AgentConnectionCheck {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(_) => {
            return AgentConnectionCheck::failed(AgentFailure::new(
                Stage::Launch,
                Reason::Configuration,
                "Cannot determine the runtime working directory",
            ));
        }
    };
    let mut options = crate::driver::options(config, cwd);
    crate::agents::apply_managed_pi_defaults(config, &mut options.env).await;
    let connection = match AcpConnection::connect_observed(
        &options,
        Arc::new(crate::observer::CallbackObserver(Arc::new(|_| {}))),
        Some(trace),
    )
    .await
    {
        Ok(connection) => connection,
        Err(error) => {
            return AgentConnectionCheck::failed(
                crate::failure::transport(error, Stage::Initialize).failure_at(Stage::Initialize),
            );
        }
    };
    let mut capabilities = connection.capabilities();
    let feedback = companion
        .map(crate::feedback_transport::validate_companion)
        .transpose();
    capabilities.feedback_transport = feedback
        .as_ref()
        .ok()
        .and_then(|path| path.as_ref())
        .map(|_| FeedbackTransport::Command);
    let mut result = AgentConnectionCheck::connected(&capabilities);
    if let Err(error) = feedback {
        result.failure = Some(AgentFailure::new(
            Stage::Launch,
            Reason::Configuration,
            error.message.clone(),
        ));
        result.message = error.message;
    }
    if let Err(error) = connection.shutdown().await {
        result.ok = false;
        result.failure = Some(AgentFailure::new(
            Stage::Initialize,
            Reason::Connection,
            "ACP connected, but the diagnostic process could not close cleanly",
        ));
        result.message = "ACP connected, but the diagnostic process could not close cleanly".into();
        result.details.push(error.to_string());
    }
    result
}
