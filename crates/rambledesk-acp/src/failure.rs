//! Map only established wire codes and local failure facts into safe diagnoses.
use rambledesk_core::{AgentDriverError, AgentFailureReason as Reason, AgentFailureStage as Stage};

pub(crate) fn protocol(
    stage: Stage,
    error: agent_client_protocol::Error,
    model: bool,
) -> AgentDriverError {
    use agent_client_protocol::ErrorCode;
    let reason = match error.code {
        ErrorCode::AuthRequired => Reason::Authentication,
        ErrorCode::InvalidParams if model => Reason::Model,
        ErrorCode::InvalidParams => Reason::Configuration,
        ErrorCode::ResourceNotFound if model => Reason::Model,
        _ => Reason::Unknown,
    };
    let message = match reason {
        Reason::Authentication => {
            "The Agent requires authentication; sign in or configure its API key, then retry this operation"
        }
        Reason::Model => {
            "The Agent rejected the selected model; choose another advertised model and retry"
        }
        Reason::Configuration => {
            "The Agent rejected the configuration values; review its options and retry"
        }
        _ => match stage {
            Stage::Launch => "The Agent could not start",
            Stage::Initialize => "The Agent did not complete ACP initialization",
            Stage::Session => "The Agent could not open this session",
            Stage::Configuration => "The Agent could not apply the selected option",
            Stage::Prompt => {
                "The Agent could not complete this message; inspect the session before retrying"
            }
        },
    };
    // The code is bounded protocol metadata. Agent-supplied message/data may
    // contain credentials or provider responses and never leave this package.
    AgentDriverError::classified(
        stage,
        reason,
        format!("{message} (ACP error {})", i32::from(error.code)),
    )
}

pub(crate) fn transport(error: crate::AcpError, stage: Stage) -> AgentDriverError {
    if let crate::AcpError::RequestFailure(_, code) = &error {
        return protocol(stage, agent_client_protocol::Error::new(*code, ""), false);
    }
    let (stage, reason) = match &error {
        crate::AcpError::InvalidLaunch(_) => (Stage::Launch, Reason::Configuration),
        crate::AcpError::Io(_) if stage == Stage::Initialize => (Stage::Launch, Reason::Connection),
        crate::AcpError::Io(_) | crate::AcpError::Closed | crate::AcpError::Timeout(_) => {
            (stage, Reason::Connection)
        }
        crate::AcpError::AuthenticationRequired(_) => (stage, Reason::Authentication),
        crate::AcpError::CannotLoad => (Stage::Session, Reason::Configuration),
        _ => (stage, Reason::Unknown),
    };
    AgentDriverError::classified(stage, reason, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_only_explicit_protocol_evidence_and_never_returns_raw_diagnostics() {
        for (code, model, expected) in [
            (-32000, false, Reason::Authentication),
            (-32602, true, Reason::Model),
            (-32602, false, Reason::Configuration),
            (-32603, false, Reason::Unknown),
        ] {
            let error = agent_client_protocol::Error::new(code, "login API_KEY=secret")
                .data(serde_json::json!({"apiKey":"secret"}));
            let error = protocol(Stage::Configuration, error, model);
            assert_eq!(error.failure.as_ref().unwrap().reason, expected);
            assert!(!error.message.contains("secret"));
        }
    }
}
