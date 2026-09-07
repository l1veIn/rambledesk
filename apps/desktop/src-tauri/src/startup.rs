use std::error::Error;

use rambledesk_storage::StorageOpenError;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

#[derive(Clone, Copy, Debug)]
pub(crate) enum StartupPhase {
    Windows,
    Configuration,
    Storage,
    Services,
    Agents,
    LocalServer,
    Tray,
    Recovery,
}

impl StartupPhase {
    fn code(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Configuration => "configuration",
            Self::Storage => "storage",
            Self::Services => "services",
            Self::Agents => "agents",
            Self::LocalServer => "local_server",
            Self::Tray => "tray",
            Self::Recovery => "recovery",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Windows => "创建窗口",
            Self::Configuration => "读取本机配置",
            Self::Storage => "打开数据资料库",
            Self::Services => "初始化服务",
            Self::Agents => "初始化 Agent 目录",
            Self::LocalServer => "启动本地服务",
            Self::Tray => "创建系统托盘",
            Self::Recovery => "恢复会话",
        }
    }
}

/// Only fixed classifications and numeric schema versions cross the startup
/// boundary. Error messages and their sources can contain paths or credentials.
#[derive(Clone, Debug)]
pub(crate) struct StartupFailure {
    phase: StartupPhase,
    code: &'static str,
    advice: &'static str,
    schema_versions: Option<(u64, u64)>,
}

impl StartupFailure {
    fn classify(phase: StartupPhase, error: &(dyn Error + 'static)) -> Self {
        let mut failure = Self {
            phase,
            code: "startup_failed",
            advice: "请关闭其他 RambleDesk 进程后重试；若仍失败，请把诊断日志和发生时间交给开发者。",
            schema_versions: None,
        };
        let mut source = Some(error);
        // A buggy external error implementation can return a cyclic source.
        for _ in 0..16 {
            let Some(error) = source else { break };
            if let Some(storage) = error.downcast_ref::<StorageOpenError>() {
                match storage {
                    StorageOpenError::NewerDatabase { applied, supported } => {
                        failure.code = "database_version_unsupported";
                        failure.schema_versions = Some((*applied, *supported));
                        failure.advice = "数据来自更新版本，本版本无法安全打开。请使用更新后的 RambleDesk；如需回退，请先联系开发者确认兼容版本或备份恢复步骤。请勿删除数据库文件。";
                    }
                    StorageOpenError::CreateDirectory(_)
                    | StorageOpenError::SecurePath(_)
                    | StorageOpenError::DataDirectoryUnavailable => {
                        failure.code = "data_directory_unavailable";
                        failure.advice = "请检查数据目录是否仍然存在、磁盘是否已连接，以及当前账户是否有读写权限，然后重新启动。";
                    }
                    StorageOpenError::Connect(_) => {
                        failure.code = "database_open_failed";
                        failure.advice = "请关闭其他 RambleDesk 进程，检查磁盘空间和数据目录的读写权限后重试。若仍失败，请保留数据库并提供诊断日志。";
                    }
                    StorageOpenError::BackupDatabase(_) | StorageOpenError::ManageBackup(_) => {
                        failure.code = "database_backup_failed";
                        failure.advice = "升级前的数据库备份未能完成。请检查磁盘空间和目录权限后重试；请勿删除现有数据库或备份。";
                    }
                    StorageOpenError::Migrate(_)
                    | StorageOpenError::RepairMigrationChecksums(_)
                    | StorageOpenError::InspectSchema(_) => {
                        failure.code = "database_migration_failed";
                        failure.advice = "数据库结构检查或升级失败。请保留数据库和备份，并把诊断日志交给开发者；请勿手动删除数据库或修改版本记录。";
                    }
                    StorageOpenError::Recovery(_) => {
                        failure.code = "database_recovery_failed";
                        failure.advice = "未能恢复资料库中的未完成操作。请保留数据库和备份，并把诊断日志交给开发者。";
                    }
                }
                break;
            }
            source = error.source();
        }
        failure
    }

    fn message(&self, logs: &str) -> String {
        let versions = self
            .schema_versions
            .map_or_else(String::new, |(applied, supported)| {
                format!("\n数据库版本：{applied}；当前应用最高支持：{supported}。")
            });
        format!(
            "RambleDesk 无法完成启动。\n\n失败阶段：{}\n错误代码：{}{versions}\n\n{}\n\n诊断日志目录：\n{logs}",
            self.phase.label(),
            self.code,
            self.advice,
        )
    }

    fn log(&self) {
        tracing::error!(
            phase = self.phase.code(),
            error_code = self.code,
            applied_schema = self.schema_versions.map(|versions| versions.0),
            supported_schema = self.schema_versions.map(|versions| versions.1),
            "RambleDesk setup failed"
        );
    }
}

pub(crate) fn fatal_startup_message(error: &(dyn Error + 'static)) -> String {
    let failure = StartupFailure::classify(StartupPhase::Windows, error);
    failure.log();
    failure.message(&crate::logging::directory_hint())
}

pub(crate) fn show_setup_failure<R: Runtime>(
    app: &AppHandle<R>,
    phase: StartupPhase,
    error: &(dyn Error + 'static),
) {
    let failure = StartupFailure::classify(phase, error);
    failure.log();
    let title = format!("RambleDesk · 启动失败 ({})", failure.code);
    let message = failure.message(&crate::logging::directory_hint());
    app.manage(failure);

    // Keep the main window reachable if the operating system cannot show the
    // dialog. The frontend can display its own timed startup error, and closing
    // this window exits even if setup installed the normal hide-to-tray handler.
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(&title);
        let _ = window.show();
        let _ = window.set_focus();
    }
    let handle = app.clone();
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "打开日志目录并退出".into(),
            "退出".into(),
        ))
        .show(move |open_logs| {
            if open_logs {
                match crate::logging::directory() {
                    Ok(directory) => {
                        if handle
                            .opener()
                            .open_path(directory.to_string_lossy(), None::<&str>)
                            .is_err()
                        {
                            tracing::warn!(
                                error_code = "startup_logs_open_failed",
                                "failed to open startup log directory"
                            );
                        }
                    }
                    Err(_) => tracing::warn!(
                        error_code = "startup_logs_unavailable",
                        "startup log directory unavailable"
                    ),
                }
            }
            handle.exit(1);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_database_explains_versions_and_safe_recovery() {
        let error = StorageOpenError::NewerDatabase {
            applied: 20,
            supported: 10,
        };
        let failure = StartupFailure::classify(StartupPhase::Storage, &error);
        assert_eq!(failure.code, "database_version_unsupported");
        assert_eq!(failure.schema_versions, Some((20, 10)));
        let message = failure.message("logs");
        assert!(message.contains("数据库版本：20；当前应用最高支持：10"));
        assert!(message.contains("请勿删除数据库文件"));
    }

    #[test]
    fn directory_failure_omits_private_io_error_details() {
        let error = StorageOpenError::CreateDirectory(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "C:/private-user/api_key=secret",
        ));
        let failure = StartupFailure::classify(StartupPhase::Storage, &error);
        assert_eq!(failure.code, "data_directory_unavailable");
        let message = failure.message("logs");
        assert!(message.contains("读写权限"));
        assert!(!message.contains("private-user"));
        assert!(!message.contains("secret"));
    }

    #[test]
    fn classifies_wrapped_storage_error_without_parsing_text() {
        let cause = anyhow::Error::new(StorageOpenError::NewerDatabase {
            applied: 21,
            supported: 20,
        })
        .context("private metadata");
        let failure = StartupFailure::classify(StartupPhase::Storage, cause.as_ref());
        assert_eq!(failure.schema_versions, Some((21, 20)));
        assert!(!failure.message("logs").contains("private metadata"));
    }

    #[test]
    fn unknown_error_retains_stage_without_serializing_cause() {
        let failure = StartupFailure::classify(
            StartupPhase::LocalServer,
            &std::io::Error::other("Bearer private-token"),
        );
        assert_eq!(failure.code, "startup_failed");
        let message = failure.message("logs");
        assert!(message.contains("启动本地服务"));
        assert!(!message.contains("private-token"));
    }
}
