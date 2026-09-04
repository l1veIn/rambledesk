//! ACP client implementation. Protocol and subprocess details stay outside core.
mod activity_content;
pub mod agents;
mod connection;
mod disconnect;
mod driver;
mod feedback_transport;
mod observer;
mod permission_details;
mod permissions;
mod process;
mod prompt_content;
mod session_configuration;

pub use connection::{AcpConnection, AcpError, AcpEvent, AcpLaunch, AcpSessionInfo};
pub use driver::{AcpSessionDriver, ConfiguredAcpSessionDriver};
