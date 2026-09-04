//! ACP client implementation. Protocol and subprocess details stay outside core.
mod connection;
mod disconnect;
mod driver;
mod observer;
mod permissions;
mod process;

pub use connection::{AcpConnection, AcpError, AcpEvent, AcpLaunch, AcpSessionInfo};
pub use driver::AcpSessionDriver;
