//! ACP client implementation. Protocol and subprocess details stay outside core.
mod connection;
mod process;

pub use connection::{AcpConnection, AcpError, AcpEvent, AcpLaunch, AcpSessionInfo};
