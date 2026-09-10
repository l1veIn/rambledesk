//! Content-free diagnostics shared by managed-session and ACP boundaries.
use sha2::{Digest, Sha256};
use std::time::Instant;

/// This guard never receives command lines, environment values or error messages.
/// Dropped futures receive a terminal marker without introducing cleanup work.
pub struct AgentOperationTrace {
    operation: &'static str,
    operation_id: String,
    subject_id: String,
    started: Instant,
    finished: bool,
    responses: u64,
}

impl AgentOperationTrace {
    pub fn new(operation: &'static str, subject_id: Option<&str>) -> Self {
        let trace = Self {
            operation,
            operation_id: uuid::Uuid::now_v7().to_string(),
            subject_id: subject_id.map(diagnostic_id).unwrap_or_default(),
            started: Instant::now(),
            finished: false,
            responses: 0,
        };
        trace.emit("started", "running", "");
        trace
    }

    fn emit(&self, phase: &'static str, status: &'static str, error_code: &'static str) {
        tracing::info!(target: "rambledesk::agents",
            operation = self.operation, operation_id = %self.operation_id,
            subject_id = %self.subject_id, phase, status, error_code,
            elapsed_ms = self.started.elapsed().as_millis() as u64,
            response_count = self.responses, "agent operation");
    }

    pub fn checkpoint(&self, phase: &'static str, status: &'static str) {
        self.emit(phase, status, "");
    }

    pub fn id(&self) -> &str {
        &self.operation_id
    }

    pub fn link(&self, role: &'static str, id: &str) {
        tracing::info!(target: "rambledesk::agents",
            operation = self.operation, operation_id = %self.operation_id,
            subject_id = %self.subject_id, phase = "linked", role,
            related_id = %diagnostic_id(id), "agent operation identity");
    }

    /// Count accepted updates, but emit only once per turn regardless of chunks.
    pub fn response(&mut self) {
        self.responses = self.responses.saturating_add(1);
        if self.responses == 1 {
            self.emit("first_response", "received", "");
        }
    }

    pub fn finish(&mut self, status: &'static str, error_code: &'static str) {
        if !self.finished {
            self.finished = true;
            self.emit("finished", status, error_code);
        }
    }

    pub fn result<T, E>(
        mut self,
        result: Result<T, E>,
        code: fn(&E) -> &'static str,
    ) -> Result<T, E> {
        match &result {
            Ok(_) => self.finish("succeeded", ""),
            Err(error) => {
                let code = code(error);
                self.finish(
                    match code {
                        "cancelled" => "cancelled",
                        "interrupted" => "interrupted",
                        "timeout" => "timed_out",
                        _ => "failed",
                    },
                    code,
                );
            }
        }
        result
    }
}

impl Drop for AgentOperationTrace {
    fn drop(&mut self) {
        self.finish("interrupted", "future_dropped");
    }
}

// Internal UUIDs remain useful for correlation. Caller-supplied/non-UUID ids are
// stable digests so even an invalid id carrying a path or secret is never logged.
fn diagnostic_id(value: &str) -> String {
    uuid::Uuid::parse_str(value)
        .map(|id| id.to_string())
        .unwrap_or_else(|_| {
            format!(
                "sha256:{}",
                hex::encode(&Sha256::digest(value.as_bytes())[..12])
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
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
    fn capture(run: impl FnOnce()) -> String {
        // Tracing's single-dispatcher optimization registers callsites against
        // the emitting thread's default. A parallel test without a subscriber
        // can therefore cache `never` for our shared production callsite. Keep
        // both the idle and recording dispatchers registered so interest is
        // aggregated and then checked against the current thread's subscriber.
        // Serialize only the capture buffer; other tests remain parallel.
        static RECORDER: OnceLock<(tracing::Dispatch, Capture, tracing::Dispatch)> =
            OnceLock::new();
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
    fn boundaries_are_paired_and_streaming_emits_only_one_first_response() {
        let log = capture(|| {
            let mut trace = AgentOperationTrace::new("session.turn", Some("session-one"));
            for _ in 0..1000 {
                trace.response();
            }
            trace.finish("succeeded", "");
            trace.finish("failed", "must_not_appear");
        });
        assert_eq!(log.lines().count(), 3);
        assert_eq!(log.matches("first_response").count(), 1);
        assert!(log.contains("response_count=1000"));
        assert!(!log.contains("must_not_appear"));
        assert!(!log.contains("future_dropped"));
    }

    #[test]
    fn errors_and_dropped_futures_are_distinct_without_logging_error_or_input_content() {
        let private = "D:/private/project API_KEY=secret prompt text";
        let log = capture(|| {
            let trace = AgentOperationTrace::new("agent.inspect", Some(private));
            trace.link("session", private);
            let _: Result<(), &str> = trace.result(Err(private), |_| "timeout");
            let _dropped = AgentOperationTrace::new("agent.install", None);
        });
        assert!(!log.contains(private));
        assert!(!log.contains("API_KEY"));
        assert!(log.contains("sha256:"));
        assert!(log.contains("timed_out"));
        assert!(log.contains("future_dropped"));
        assert_eq!(log.matches("phase=\"started\"").count(), 2);
        assert_eq!(log.matches("phase=\"finished\"").count(), 2);
    }

    #[test]
    fn diagnostic_identifiers_are_stable_and_uuid_correlation_is_preserved() {
        let id = uuid::Uuid::now_v7().to_string();
        assert_eq!(diagnostic_id(&id), id);
        assert_eq!(diagnostic_id("claude-acp"), diagnostic_id("claude-acp"));
        assert_ne!(diagnostic_id("claude-acp"), diagnostic_id("pi-acp"));
    }

    #[test]
    fn capture_survives_first_callsite_use_on_a_thread_without_a_subscriber() {
        fn emit_probe() {
            tracing::info!(target: "rambledesk::agents", "capture registration probe");
        }
        assert!(capture(|| {}).is_empty());
        std::thread::spawn(emit_probe).join().unwrap();
        let log = capture(emit_probe);
        assert_eq!(log.lines().count(), 1);
        assert!(log.contains("capture registration probe"));
    }
}
