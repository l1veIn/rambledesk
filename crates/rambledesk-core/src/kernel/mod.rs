//! ACP-first RambleDesk domain kernel.
//!
//! This module is intentionally self-contained while the v2 application is
//! frozen. Its public surface is the durable Core Interface plus the two
//! Adapter seams in [`ports`]. Protocol and filesystem details do not belong
//! here.

mod core;
mod core_artifacts;
mod core_support;
mod digest;
mod error;
mod model;
pub mod ports;

pub use core::Core;
pub use error::{CoreError, CoreErrorCode};
pub use model::*;

#[cfg(test)]
mod test_adapters;
#[cfg(test)]
mod test_fixtures;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_behavior;
#[cfg(test)]
mod tests_contract;
#[cfg(test)]
mod tests_determinism;
