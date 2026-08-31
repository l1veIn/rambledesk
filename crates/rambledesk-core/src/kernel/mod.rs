//! RambleDesk v3 domain kernel for Managed ACP Sessions and Imported Sessions.
//!
//! This module is intentionally self-contained while the v2 application is
//! frozen. Its public surface is the durable Core Interface plus the two
//! Adapter seams in [`ports`]. Protocol and filesystem details do not belong
//! here.

mod core;
mod core_artifacts;
mod core_sessions;
mod core_support;
mod digest;
mod error;
mod model;
pub mod ports;

pub use core::Core;
pub use core_support::{
    validate_feedback_request_input, validate_feedback_submission_input,
    validate_ramble_draft_content,
};
pub use digest::{
    PackageDigestInput, PackageDigests, calculate_feedback_request_digest,
    calculate_feedback_submission_digest, calculate_package_digests, package_digests_match,
};
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
