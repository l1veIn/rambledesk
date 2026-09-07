//! ACP client implementation. Protocol and subprocess details stay outside core.
mod activity_content;
pub mod agents;
mod connection;
mod disconnect;
mod driver;
mod feedback_transport;
mod feedback_workflow;
mod observer;
mod permission_details;
mod permissions;
mod process;
mod prompt_content;
mod session_configuration;
mod user_input;

#[cfg(test)]
mod diagnostic_tests;

/// Low-level protocol access for diagnostics and transport probes.
/// Application sessions should use the drivers exported at the crate root.
pub mod probe {
    pub use crate::connection::{AcpConnection, AcpError, AcpEvent, AcpLaunch, AcpSessionInfo};
}

pub(crate) use connection::{AcpConnection, AcpError, AcpEvent, AcpLaunch};
pub use driver::{AcpSessionDriver, ConfiguredAcpSessionDriver};
