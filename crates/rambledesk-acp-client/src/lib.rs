//! Managed Agent Client Protocol runtime for RambleDesk.
//!
//! The external Interface is deliberately smaller than the ACP wire surface:
//! callers reconcile durable Sessions, answer live human-attention requests,
//! cancel a turn, subscribe to live projections, and shut the runtime down.
//! JSON-RPC correlation, capability negotiation, process ownership and
//! session recovery remain inside this Module.

mod catalog;
mod client;
mod elicitation;
mod error;
mod launch_schema;
mod process;
mod rpc;
mod toolset;
mod types;

pub use catalog::{
    BUILTIN_AGENTS, BinaryDirectoryEntry, BuiltinAccessModes, BuiltinAgentDistribution,
    BuiltinAgentSpec, PlatformArtifact, PlatformFiles, builtin_agent, builtin_agents,
};
pub use client::AcpClient;
pub use error::{AcpClientError, AcpErrorCode};
pub use types::*;
