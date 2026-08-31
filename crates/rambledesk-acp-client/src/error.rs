use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AcpErrorCode {
    InvalidArgument,
    LaunchProfileNotFound,
    SessionNotFound,
    SessionNotManaged,
    RunDisconnected,
    LiveRequestNotFound,
    LiveRequestNotCurrent,
    InvalidLiveAnswer,
    UnsupportedAccessMode,
    UnsupportedCapability,
    SessionToolsetUnsupported,
    AuthenticationRequired,
    AgentLaunchFailed,
    ProtocolViolation,
    RpcError,
    OperationTimedOut,
    CoreFailure,
    ShutdownFailed,
}

#[derive(Debug, Error)]
#[error("{code:?}: {message}")]
pub struct AcpClientError {
    pub code: AcpErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl AcpClientError {
    pub(crate) fn new(code: AcpErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }

    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::new(AcpErrorCode::InvalidArgument, message, false)
    }

    pub(crate) fn protocol(message: impl Into<String>) -> Self {
        Self::new(AcpErrorCode::ProtocolViolation, message, false)
    }

    pub(crate) fn disconnected(message: impl Into<String>) -> Self {
        Self::new(AcpErrorCode::RunDisconnected, message, true)
    }
}
