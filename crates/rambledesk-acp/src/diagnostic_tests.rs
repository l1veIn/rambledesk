use crate::{AcpConnection, AcpError, AcpLaunch};
use std::{
    collections::BTreeMap,
    io::Write,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Tracing caches one interest value per callsite and recomputes it whenever a
/// scoped dispatcher appears, taking the `and` of every dispatcher that is still
/// alive. A rebuild that only sees the global `NoSubscriber` therefore caches
/// `never`, and later events from the same callsite are dropped before the
/// emitting thread's subscriber is consulted: a capture can come back empty even
/// though the code under test ran. Keeping a recording dispatcher and an idle
/// `NoSubscriber` alive for the whole test binary aggregates their interests into
/// `sometimes`, so every event is decided by the current thread's subscriber.
/// This mirrors `rambledesk_core::agent_operation_trace`'s own capture helper.
fn capture(run: impl FnOnce()) -> String {
    static RECORDER: OnceLock<(tracing::Dispatch, Capture, tracing::Dispatch)> = OnceLock::new();
    static CAPTURE_LOCK: Mutex<()> = Mutex::new(());
    let _capture = CAPTURE_LOCK.lock().unwrap();
    let (dispatcher, output, _idle) = RECORDER.get_or_init(|| {
        let idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        let output = Capture::default();
        let writer = output.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        (tracing::Dispatch::new(subscriber), output, idle)
    });
    output.0.lock().unwrap().clear();
    tracing::dispatcher::with_default(dispatcher, run);
    String::from_utf8(output.0.lock().unwrap().clone()).unwrap()
}

#[test]
fn spawn_failure_records_paired_boundaries_without_launch_or_error_payloads() {
    let temp = tempfile::tempdir().unwrap();
    let launch = AcpLaunch {
        command: temp
            .path()
            .join("private-command-unavailable")
            .to_string_lossy()
            .into_owned(),
        args: vec!["private-prompt-do-not-log".into()],
        cwd: temp.path().to_path_buf(),
        env: BTreeMap::from([("API_KEY".into(), "private-api-key-do-not-log".into())]),
        mcp_servers: vec![],
    };
    let log = capture(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                assert!(
                    AcpConnection::connect(&launch, Arc::new(|_| {}))
                        .await
                        .is_err()
                );
            });
    });
    assert_eq!(log.matches("phase=\"started\"").count(), 2);
    assert_eq!(log.matches("phase=\"finished\"").count(), 2);
    assert!(log.contains("acp.spawn"));
    assert!(log.contains("status=\"failed\""));
    assert!(!log.contains("private-"));
    assert!(!log.contains("API_KEY"));
    assert!(!log.contains(&temp.path().to_string_lossy().into_owned()));
}

#[test]
fn diagnostic_error_codes_do_not_format_error_payloads() {
    assert_eq!(
        AcpError::InvalidLaunch("private-launch-path".into()).diagnostic_code(),
        "invalid_launch"
    );
    assert_eq!(
        AcpError::Io(std::io::Error::other("private-stderr")).diagnostic_code(),
        "process_io"
    );
    assert_eq!(AcpError::Timeout("initialize").diagnostic_code(), "timeout");
    assert_eq!(AcpError::Closed.diagnostic_code(), "closed");
    assert_eq!(
        crate::agents::CatalogError::Timeout.diagnostic_code(),
        "timeout"
    );
    assert_eq!(
        crate::agents::CatalogError::Cancelled.diagnostic_code(),
        "cancelled"
    );
    assert_eq!(
        crate::agents::CatalogError::NodeVersion.diagnostic_code(),
        "node_version"
    );
}

#[test]
fn repeated_eof_reads_emit_only_one_disconnect_and_no_second_terminal_on_drop() {
    let log = capture(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                use tokio::io::AsyncReadExt;
                let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let mut reader = crate::disconnect::DisconnectReader {
                    inner: tokio::io::empty(),
                    closed: closed.clone(),
                    trace: rambledesk_core::agent_operation_trace::AgentOperationTrace::new(
                        "acp.transport",
                        None,
                    ),
                };
                assert_eq!(reader.read(&mut [0u8; 1]).await.unwrap(), 0);
                assert_eq!(reader.read(&mut [0u8; 1]).await.unwrap(), 0);
                assert!(closed.load(std::sync::atomic::Ordering::SeqCst));
            });
    });
    assert_eq!(log.lines().count(), 2);
    assert_eq!(log.matches("status=\"disconnected\"").count(), 1);
    assert!(log.contains("error_code=\"eof\""));
    assert!(!log.contains("reader_dropped"));
}
