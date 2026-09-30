use super::*;
pub type WorkbenchPrePublish = fn(&WorkbenchRuntimeContext<'_>) -> Result<(), ApplicationError>;
pub struct WorkbenchDefinition {
    pub summary: WorkbenchSummary,
    pub advertised: bool,
    pub approval: bool,
    pub legacy_identity: bool,
    pub strict_state: bool,
    pub validate_saved_draft: bool,
    pub notes_only: bool,
    pub require_complete: bool,
    pub describe: fn() -> Result<WorkbenchDescription, ApplicationError>,
    pub pre_publish: Option<WorkbenchPrePublish>,
}
impl WorkbenchDefinition {
    pub fn new(
        summary: WorkbenchSummary,
        describe: fn() -> Result<WorkbenchDescription, ApplicationError>,
    ) -> Self {
        Self {
            summary,
            advertised: true,
            approval: false,
            legacy_identity: false,
            strict_state: false,
            validate_saved_draft: true,
            notes_only: false,
            require_complete: true,
            describe,
            pre_publish: None,
        }
    }
    pub fn validate_runtime(
        &self,
        context: &WorkbenchRuntimeContext<'_>,
    ) -> Result<(), ApplicationError> {
        match self.pre_publish {
            Some(check) => check(context),
            None => Ok(()),
        }
    }
}
pub struct WorkbenchRuntimeContext<'a> {
    pub request_id: &'a str,
    pub(crate) terminals: &'a crate::TerminalSessionManager,
}
impl WorkbenchRuntimeContext<'_> {
    pub fn terminal_is_finished(&self) -> bool {
        self.terminals.request_is_finished(self.request_id)
    }
}

impl crate::FeedbackApplication {
    pub(crate) async fn prepare_workbench_publication(
        &self,
        request_id: &str,
        expected_revision: u64,
    ) -> Result<Option<tokio::sync::OwnedMutexGuard<()>>, ApplicationError> {
        let workspace = self.get_feedback_workspace(request_id.to_owned()).await?;
        let Some(spec) = workspace.workbench.as_ref() else {
            return Ok(None);
        };
        let definition = validate_workbench(spec)
            .map_err(|_| ApplicationError::from(crate::RepositoryError::WorkbenchUnsupported))?
            .kind()
            .definition();
        if definition.pre_publish.is_none() {
            return Ok(None);
        }
        if workspace.request.revision != expected_revision {
            return Err(ApplicationError::from(
                crate::RepositoryError::DraftConflict,
            ));
        }
        // Resource creation and publication share this gate. A new PTY cannot
        // race the runtime check and appear while an immutable package publishes.
        let guard = self.terminal_sessions.lifecycle_guard().await;
        definition.validate_runtime(&WorkbenchRuntimeContext {
            request_id,
            terminals: &self.terminal_sessions,
        })?;
        Ok(Some(guard))
    }
}
macro_rules! register_workbenches {
    ($( $(#[$attribute:meta])* $variant:ident => $module:ident {
        wire: $wire:literal, data: $data:ident,
        state: [$($state:tt)*],
        result: $result:tt,
        exports: [$($export:ident),* $(,)?]
    })*) => {
        $( $(#[$attribute])* mod $module;
           $(#[$attribute])* pub use $module::{$($export),*};
        )*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum WorkbenchKind { $( $(#[$attribute])* $variant, )* }
        impl WorkbenchKind {
            pub const ALL: &'static [Self] = &[$($(#[$attribute])* Self::$variant,)*];
            pub fn as_str(self) -> &'static str { match self { $($(#[$attribute])* Self::$variant => $wire,)* } }
            pub fn resolve(kind: &str, version: u32) -> Option<Self> {
                Self::ALL.iter().copied().find(|item| version == 1 && item.as_str() == kind)
            }
            pub fn definition(self) -> WorkbenchDefinition {
                match self { $($(#[$attribute])* Self::$variant => $module::definition(),)* }
            }
            pub fn supports_approval(self) -> bool { self.definition().approval }
            pub fn uses_legacy_action_identity(self) -> bool { self.definition().legacy_identity }
            pub(crate) fn decode_data(self, raw: serde_value::Value) -> Option<WorkbenchData> {
                match self { $($(#[$attribute])* Self::$variant => raw.deserialize_into().ok().map(WorkbenchData::$variant),)* }
            }
            pub(crate) fn decode_result(self, raw: serde_value::Value) -> Option<WorkbenchResult> {
                match self { $($(#[$attribute])* Self::$variant => register_workbenches!(@decode_result $variant $result raw),)* }
            }
        }
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
        #[serde(untagged)]
        pub enum WorkbenchData {
            $($(#[$attribute])* $variant($data),)*
            Unknown(#[ts(type = "Record<string, unknown>")] serde_value::Value),
        }
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
        #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
        pub enum WorkbenchState { $($($state)*)* }
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
        #[serde(untagged, deny_unknown_fields)]
        pub enum WorkbenchResult {
            $($(#[$attribute])* $variant $result,)*
            Unknown(#[ts(type = "Record<string, unknown>")]
                #[schemars(schema_with = "opaque_workbench_schema")] serde_value::Value),
        }
        pub enum ValidatedWorkbench<'a> { $($(#[$attribute])* $variant(&'a $data),)* }
        impl ValidatedWorkbench<'_> {
            pub fn kind(&self) -> WorkbenchKind {
                match self { $($(#[$attribute])* Self::$variant(_) => WorkbenchKind::$variant,)* }
            }
            pub fn legacy_capture_actions(&self) -> Vec<ActionInput> {
                match self { $($(#[$attribute])* Self::$variant(data) => $module::legacy_actions(data),)* }
            }
            pub(crate) fn result(&self, state: Option<&WorkbenchState>) -> Option<WorkbenchResult> {
                match self { $($(#[$attribute])* Self::$variant(data) => $module::result(data, state),)* }
            }
            pub(crate) fn draft_valid(&self, state: Option<&WorkbenchState>) -> bool {
                match self { $($(#[$attribute])* Self::$variant(data) => $module::draft_valid(data, state),)* }
            }
            pub(crate) fn has_input(&self, state: Option<&WorkbenchState>) -> bool {
                match self { $($(#[$attribute])* Self::$variant(data) => $module::has_input(data, state),)* }
            }
            pub(crate) fn complete(&self, result: Option<&WorkbenchResult>) -> bool {
                match self { $($(#[$attribute])* Self::$variant(data) => $module::complete(data, result),)* }
            }
            pub(crate) fn has_result_input(&self, result: Option<&WorkbenchResult>) -> bool {
                match self { $($(#[$attribute])* Self::$variant(_) => $module::result_has_input(result),)* }
            }
        }
        pub fn validate_workbench(spec: &WorkbenchSpec) -> Result<ValidatedWorkbench<'_>, ApplicationError> {
            let kind = WorkbenchKind::resolve(&spec.kind, spec.version).ok_or_else(unsupported)?;
            match (kind, &spec.data) {
                $($(#[$attribute])* (WorkbenchKind::$variant, WorkbenchData::$variant(data)) => {
                    $module::validate(data)?; Ok(ValidatedWorkbench::$variant(data))
                },)*
                _ => Err(ApplicationError::invalid_argument(
                    "workbench.data does not match its type; call describe_workbench for its schema")),
            }
        }
        pub fn workbench_type_declarations() -> Vec<String> {
            fn declaration<T: TS>() -> String {
                T::decl(&ts_rs::Config::default()).replacen("type ", "export type ", 1)
                    .lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
            }
            let mut types = vec![declaration::<WorkbenchSpec>(), declaration::<WorkbenchData>(),
                declaration::<WorkbenchState>(), declaration::<WorkbenchPackage>(), declaration::<WorkbenchResult>()];
            $($(#[$attribute])* { $(types.push(declaration::<$export>());)* })*
            types
        }
    };
    (@decode_result $variant:ident ($result:ty) $raw:ident) => {
        $raw.deserialize_into::<$result>().ok().map(WorkbenchResult::$variant)
    };
    (@decode_result $variant:ident {$($field:ident: $field_type:ty),* $(,)?} $raw:ident) => {{
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireResult { $($field: $field_type),* }
        $raw.deserialize_into::<WireResult>().ok().map(|result| WorkbenchResult::$variant {
            $($field: result.$field),*
        })
    }};
}
pub(super) fn unsupported() -> ApplicationError {
    ApplicationError::invalid_argument(
        "Unknown workbench type or unsupported version. Call list_workbenches to discover supported types.",
    )
}
