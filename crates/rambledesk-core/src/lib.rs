//! Framework-independent RambleDesk domain and application contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SERVICE_NAME: &str = "rambledesk";
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ServiceStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum StorageStatus {
    NotInitialized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct HealthSnapshot {
    pub service_name: String,
    pub service_version: String,
    pub status: ServiceStatus,
    pub storage: StorageStatus,
}

impl HealthSnapshot {
    pub fn m0() -> Self {
        Self {
            service_name: SERVICE_NAME.to_owned(),
            service_version: SERVICE_VERSION.to_owned(),
            status: ServiceStatus::Ready,
            storage: StorageStatus::NotInitialized,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m0_health_is_stable_and_camel_case() {
        let value = serde_json::to_value(HealthSnapshot::m0()).expect("health serializes");
        assert_eq!(value["serviceName"], SERVICE_NAME);
        assert_eq!(value["status"], "ready");
        assert_eq!(value["storage"], "not_initialized");
        assert!(value.get("service_name").is_none());
    }
}
