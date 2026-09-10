//! Private configuration codecs. Session orchestration sees generic requests.
use super::*;
use serde_json::Value;
#[path = "session_configuration_grok.rs"]
mod grok;

pub(super) struct Configuration(grok::Configuration);

pub(super) struct Request {
    pub method: &'static str,
    pub params: Value,
}

impl Configuration {
    pub fn parse(raw: &Value) -> Result<Option<Self>, AcpError> {
        Ok(grok::Configuration::parse(raw)?.map(Self))
    }

    pub fn options(&self) -> &[SessionConfigOption] {
        self.0.options()
    }

    pub fn request(&self, remote: &str, id: &str, value: &SessionConfigValue) -> Option<Request> {
        self.0.request(remote, id, value)
    }

    pub fn confirmed(&mut self, id: &str, value: &SessionConfigValue) {
        self.0.confirmed(id, value);
    }
}
