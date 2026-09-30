use crate::{
    ApplicationCommandFacade, ApplicationError, FeedbackStatus, OpenTerminalSessionInput,
    ReadTerminalSessionInput, ResizeTerminalSessionInput, TerminalSessionInput,
    TerminalSessionSnapshot, WriteTerminalSessionInput,
};

impl ApplicationCommandFacade {
    async fn terminal_request(
        &self,
        request_id: &str,
        require_active: bool,
    ) -> Result<crate::FeedbackWorkspaceView, ApplicationError> {
        let workspace = self
            .application
            .get_feedback_workspace(request_id.to_owned())
            .await?;
        if require_active
            && matches!(
                workspace.request.status,
                FeedbackStatus::Completed | FeedbackStatus::Cancelled
            )
        {
            self.application
                .terminal_sessions
                .stop_request(&workspace.request.request_id);
            return Err(ApplicationError::invalid_argument(
                "The feedback request has ended; its terminal is read-only.",
            ));
        }
        let spec = workspace.workbench.as_ref().ok_or_else(|| {
            ApplicationError::invalid_argument("This request has no terminal workbench.")
        })?;
        if crate::validate_workbench(spec)?.kind() != crate::WorkbenchKind::Terminal {
            return Err(ApplicationError::invalid_argument(
                "This request has no supported terminal workbench.",
            ));
        }
        Ok(workspace)
    }

    pub async fn open_terminal_session(
        &self,
        mut input: OpenTerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        let workspace = self.terminal_request(&input.request_id, true).await?;
        input.request_id = workspace.request.request_id;
        let spec = workspace.workbench.expect("validated terminal workbench");
        let crate::ValidatedWorkbench::Terminal(data) = crate::validate_workbench(&spec)? else {
            unreachable!("validated terminal data")
        };
        self.application
            .terminal_sessions
            .open(
                input.request_id,
                data.cwd.clone(),
                data.shell.clone(),
                input.cols,
                input.rows,
            )
            .await
    }

    pub async fn read_terminal_session(
        &self,
        mut input: ReadTerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        let workspace = self.terminal_request(&input.request_id, false).await?;
        input.request_id = workspace.request.request_id;
        self.application.terminal_sessions.read(&input)
    }

    pub async fn write_terminal_session(
        &self,
        mut input: WriteTerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        let workspace = self.terminal_request(&input.request_id, true).await?;
        input.request_id = workspace.request.request_id;
        self.application.terminal_sessions.write(input).await
    }

    pub async fn resize_terminal_session(
        &self,
        mut input: ResizeTerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        let workspace = self.terminal_request(&input.request_id, true).await?;
        input.request_id = workspace.request.request_id;
        self.application.terminal_sessions.resize(input).await
    }

    pub async fn stop_terminal_session(
        &self,
        mut input: TerminalSessionInput,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        let workspace = self.terminal_request(&input.request_id, false).await?;
        input.request_id = workspace.request.request_id;
        self.application.terminal_sessions.stop(input).await
    }
}
