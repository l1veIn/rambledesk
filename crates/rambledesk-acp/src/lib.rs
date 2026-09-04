//! ACP client implementation. Protocol and subprocess details stay outside core.
mod connection;
mod driver;
mod observer;
mod process;

pub use connection::{AcpConnection, AcpError, AcpEvent, AcpLaunch, AcpSessionInfo};
pub use driver::AcpSessionDriver;
