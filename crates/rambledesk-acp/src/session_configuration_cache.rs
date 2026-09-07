use super::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Route {
    Standard(String),
    Mode,
    Model,
    Extension(String),
}

#[derive(Default)]
pub(crate) struct InitialConfiguration {
    pub(super) standard: Vec<SessionConfigOption>,
    pub(super) modes: Option<mapping::Catalog>,
    pub(super) models: Option<mapping::Catalog>,
    pub(super) extension: Option<extension::Configuration>,
}

#[derive(Default)]
pub(crate) struct ConfigurationCache {
    pub state: SessionConfiguration,
    pub(super) initial: InitialConfiguration,
    remote: Option<String>,
    early_remote: Option<String>,
    early_options: Option<Vec<SessionConfigOption>>,
    early_mode: Option<String>,
    pub(super) mode_revision: u64,
    // Retain route identities when a dependent option disappears and returns.
    // Wire IDs never share a namespace with application IDs or other routes.
    ids: HashMap<Route, String>,
    routes: HashMap<String, Route>,
}

impl ConfigurationCache {
    pub fn opened(
        &mut self,
        remote: &str,
        mut initial: InitialConfiguration,
    ) -> Result<(), AcpError> {
        if self
            .early_remote
            .as_deref()
            .is_some_and(|early| early != remote)
        {
            return Err(AcpError::Protocol("configuration attribution"));
        }
        if let Some(options) = self.early_options.take() {
            initial.standard = options;
        }
        self.initial = initial;
        if let Some(mode) = self.early_mode.take() {
            self.apply_mode(mode);
        }
        self.remote = Some(remote.into());
        self.refresh();
        Ok(())
    }

    pub fn observe(&mut self, notification: &acp::SessionNotification) -> Result<(), AcpError> {
        let (options, mode) = match &notification.update {
            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                (Some(mapping::options(&update.config_options)?), None)
            }
            acp::SessionUpdate::CurrentModeUpdate(update) => (
                None,
                Some(mapping::identifier(&update.current_mode_id.to_string())?),
            ),
            _ => return Ok(()),
        };
        let remote = notification.session_id.to_string();
        if self
            .remote
            .as_ref()
            .or(self.early_remote.as_ref())
            .is_some_and(|expected| expected != &remote)
        {
            return Err(AcpError::Protocol("configuration attribution"));
        }
        if self.remote.is_none() {
            self.early_remote = Some(remote);
            if options.is_some() {
                self.early_options = options;
            }
            if mode.is_some() {
                self.early_mode = mode;
            }
        } else {
            if let Some(options) = options {
                self.initial.standard = options;
            }
            if let Some(mode) = mode {
                self.apply_mode(mode);
                self.mode_revision = self.mode_revision.wrapping_add(1);
            }
            self.refresh();
        }
        Ok(())
    }

    pub(super) fn apply_mode(&mut self, current: String) {
        let modes = self.initial.modes.get_or_insert_with(|| mapping::Catalog {
            current: current.clone(),
            choices: vec![],
        });
        modes.current = current;
    }

    pub(super) fn route(&self, id: &str) -> Option<&Route> {
        self.routes.get(id)
    }

    pub(super) fn refresh(&mut self) {
        let mut options: Vec<_> = self
            .initial
            .standard
            .iter()
            .cloned()
            .map(|option| (Route::Standard(option.id.clone()), option))
            .collect();
        // A standard advertisement owns the extension compatibility surface.
        if options.is_empty()
            && let Some(extension) = &self.initial.extension
        {
            options.extend(
                extension
                    .options()
                    .iter()
                    .cloned()
                    .map(|option| (Route::Extension(option.id.clone()), option)),
            );
        }
        for (category, name, route, catalog) in [
            ("mode", "Mode", Route::Mode, &self.initial.modes),
            ("model", "Model", Route::Model, &self.initial.models),
        ] {
            if !options.iter().any(|(_, option)| {
                option
                    .category
                    .as_deref()
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case(category))
            }) && let Some(catalog) = catalog
            {
                options.push((route, catalog.option(category, name)));
            }
        }
        self.state.options = options
            .into_iter()
            .map(|(route, mut option)| {
                // Fresh cache identities also reject controls left over from a
                // retired connection whose advertisements may have changed order.
                let id = self
                    .ids
                    .entry(route.clone())
                    .or_insert_with(|| uuid::Uuid::now_v7().to_string())
                    .clone();
                self.routes.insert(id.clone(), route);
                option.id = id;
                option
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_normalizes_category_comparison_without_changing_its_value() {
        let mut cache = ConfigurationCache::default();
        cache
            .opened(
                "session",
                InitialConfiguration {
                    standard: vec![SessionConfigOption {
                        id: "standard-mode".into(),
                        name: "Mode".into(),
                        description: None,
                        category: Some(" MoDe ".into()),
                        kind: SessionConfigKind::Select {
                            current_value: "ask".into(),
                            options: vec![],
                        },
                    }],
                    modes: Some(mapping::Catalog {
                        current: "ask".into(),
                        choices: vec![],
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(cache.state.options.len(), 1);
        assert_eq!(cache.state.options[0].category.as_deref(), Some(" MoDe "));
    }

    #[test]
    fn stale_ids_from_another_connection_cannot_target_matching_values() {
        let open = |wire_id: &str| {
            let mut cache = ConfigurationCache::default();
            cache
                .opened(
                    "same-remote",
                    InitialConfiguration {
                        standard: vec![SessionConfigOption {
                            id: wire_id.into(),
                            name: "Setting".into(),
                            description: None,
                            category: None,
                            kind: SessionConfigKind::Boolean {
                                current_value: false,
                            },
                        }],
                        ..Default::default()
                    },
                )
                .unwrap();
            cache
        };
        let first = open("first-wire-option");
        let mut replacement = open("different-wire-option");
        let stale = SessionConfigChange {
            config_id: first.state.options[0].id.clone(),
            value: SessionConfigValue::Boolean { value: true },
        };
        assert!(first.state.allows(&stale));
        assert!(!replacement.state.allows(&stale));
        assert!(replacement.route(&stale.config_id).is_none());
        let id = replacement.state.options[0].id.clone();
        replacement.refresh();
        assert_eq!(replacement.state.options[0].id, id);
    }
}
