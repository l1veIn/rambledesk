use super::{
    ApplicationError, TerminalSessionSnapshot, TerminalSessionStatus, buffer::OutputBuffer,
    unavailable,
};
use portable_pty::{CommandBuilder, MasterPty, PtyPair, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};
use tokio::sync::{Notify, oneshot};
mod ownership;

type Reply = oneshot::Sender<Result<(), ApplicationError>>;
enum Control {
    Write(String, Reply),
    Resize(u16, u16, Reply),
    Stop,
}

struct Shared {
    snapshot: TerminalSessionSnapshot,
    output: OutputBuffer,
    reader_done: bool,
    child_done: bool,
    stop_requested: bool,
    child_exit_code: Option<i32>,
    #[cfg(windows)]
    initial_cursor_answered: bool,
}

impl Shared {
    fn refresh_completion(&mut self) {
        // A shell exit is not the end of its output stream. Keep clients polling
        // until the PTY reader has drained its final bytes before publishing it.
        if self.child_done
            && self.reader_done
            && self.snapshot.status == TerminalSessionStatus::Running
        {
            self.snapshot.status = TerminalSessionStatus::Exited;
        }
        if self.snapshot.status != TerminalSessionStatus::Running {
            self.snapshot.exit_code = self.child_exit_code;
        }
    }
}

pub(super) struct TerminalRuntime {
    id: String,
    request_id: String,
    shared: Arc<Mutex<Shared>>,
    control: mpsc::SyncSender<Control>,
    done: Arc<Notify>,
    ownership: Arc<Mutex<ownership::Ownership>>,
}

impl TerminalRuntime {
    pub fn spawn(
        request_id: &str,
        cwd: &str,
        shell: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<Self, ApplicationError> {
        let path = Path::new(cwd);
        if !path.is_absolute() || !path.is_dir() {
            return Err(unavailable(
                "Terminal working directory must be an existing absolute directory.",
            ));
        }
        let cwd = path
            .canonicalize()
            .map_err(|_| unavailable("Terminal working directory is unavailable."))?;
        let shell = shell
            .filter(|text| !text.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(default_shell);
        let pair = native_pty_system()
            .openpty(size(cols, rows))
            .map_err(|_| unavailable("Interactive terminal is unavailable on this host."))?;
        let mut command = CommandBuilder::new(&shell);
        // canonicalize() returns the extended \\?\ prefix on Windows; cmd.exe
        // interprets that as a UNC directory and silently falls back to Windows.
        command.cwd(display_path(&cwd));
        command.env("TERM", "xterm-256color");
        #[cfg(windows)]
        if shell.rsplit(['/', '\\']).next().is_some_and(|name| {
            name.eq_ignore_ascii_case("cmd.exe") || name.eq_ignore_ascii_case("cmd")
        }) {
            command.args(["/D", "/Q"]);
        }
        let mut child = match pair.slave.spawn_command(command) {
            Ok(child) => child,
            Err(_) => {
                cleanup_failed_pair(pair, None, None);
                return Err(unavailable("Terminal shell could not be started."));
            }
        };
        let mut ownership = match ownership::Ownership::new(child.as_ref(), pair.master.as_ref()) {
            Ok(ownership) => ownership,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                cleanup_failed_pair(pair, None, None);
                return Err(unavailable(
                    "Terminal process ownership could not be established.",
                ));
            }
        };
        let mut reader = match pair.master.try_clone_reader() {
            Ok(reader) => reader,
            Err(_) => {
                let _ = ownership.terminate();
                let _ = child.kill();
                let _ = child.wait();
                cleanup_failed_pair(pair, None, None);
                return Err(unavailable("Terminal output could not be opened."));
            }
        };
        let writer = match pair.master.take_writer() {
            Ok(writer) => writer,
            Err(_) => {
                let _ = ownership.terminate();
                let _ = child.kill();
                let _ = child.wait();
                cleanup_failed_pair(pair, Some(reader), None);
                return Err(unavailable("Terminal input could not be opened."));
            }
        };
        drop(pair.slave);
        let id = uuid::Uuid::now_v7().to_string();
        let shared = Arc::new(Mutex::new(Shared {
            snapshot: TerminalSessionSnapshot {
                session_id: id.clone(),
                request_id: request_id.to_owned(),
                cwd: display_path(&cwd),
                shell,
                cols,
                rows,
                status: TerminalSessionStatus::Running,
                exit_code: None,
                output: String::new(),
                first_sequence: 0,
                next_sequence: 0,
                truncated: false,
            },
            output: OutputBuffer::default(),
            reader_done: false,
            child_done: false,
            stop_requested: false,
            child_exit_code: None,
            #[cfg(windows)]
            initial_cursor_answered: false,
        }));
        let done = Arc::new(Notify::new());
        let (control, receiver) = mpsc::sync_channel(64);
        let ownership = Arc::new(Mutex::new(ownership));
        let reader_shared = shared.clone();
        let reader_done = done.clone();
        thread::spawn(move || {
            let mut bytes = [0; 8192];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(length) => reader_shared
                        .lock()
                        .expect("terminal output")
                        .output
                        .append(&bytes[..length], false),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    // Unix PTY masters report EIO when their last slave closes.
                    Err(_) => break,
                }
            }
            let mut state = reader_shared.lock().expect("terminal output");
            state.output.append(&[], true);
            state.reader_done = true;
            state.refresh_completion();
            drop(state);
            reader_done.notify_waiters();
        });
        let waiter_shared = shared.clone();
        let waiter_done = done.clone();
        let waiter_control = control.clone();
        let waiter_ownership = ownership.clone();
        thread::spawn(move || {
            let result = ownership::wait(child.as_mut(), &waiter_ownership);
            let mut state = waiter_shared.lock().expect("terminal output");
            state.child_exit_code = result.ok().map(|status| status.exit_code() as i32);
            state.child_done = true;
            state.refresh_completion();
            drop(state);
            let _ = waiter_control.send(Control::Stop);
            waiter_done.notify_waiters();
        });
        let worker_shared = shared.clone();
        let worker_ownership = ownership.clone();
        thread::spawn(move || {
            control_loop(
                pair.master,
                writer,
                worker_ownership,
                receiver,
                worker_shared,
            )
        });
        Ok(Self {
            id,
            request_id: request_id.to_owned(),
            shared,
            control,
            done,
            ownership,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    pub fn is_running(&self) -> bool {
        self.shared.lock().expect("terminal output").snapshot.status
            == TerminalSessionStatus::Running
    }
    pub fn is_finished(&self) -> bool {
        let state = self.shared.lock().expect("terminal output");
        state.reader_done && state.child_done
    }

    pub fn snapshot(
        &self,
        after: Option<u64>,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        let state = self.shared.lock().expect("terminal output");
        let (output, first_sequence) = state.output.read(after)?;
        let mut snapshot = state.snapshot.clone();
        snapshot.output = output;
        snapshot.first_sequence = first_sequence;
        snapshot.next_sequence = state.output.next;
        snapshot.truncated = state.output.truncated;
        Ok(snapshot)
    }

    pub async fn write(&self, data: String) -> Result<TerminalSessionSnapshot, ApplicationError> {
        let (reply, received) = oneshot::channel();
        self.send(Control::Write(data, reply))?;
        match tokio::time::timeout(Duration::from_secs(5), received).await {
            Ok(result) => {
                result.map_err(|_| unavailable("Terminal input is no longer available."))??
            }
            Err(_) => {
                self.request_stop();
                return Err(unavailable(
                    "Terminal input timed out; the session has been stopped.",
                ));
            }
        }
        self.snapshot(None)
    }

    pub async fn resize(
        &self,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSessionSnapshot, ApplicationError> {
        let (reply, received) = oneshot::channel();
        self.send(Control::Resize(cols, rows, reply))?;
        match tokio::time::timeout(Duration::from_secs(5), received).await {
            Ok(result) => result.map_err(|_| unavailable("Terminal is no longer available."))??,
            Err(_) => {
                self.request_stop();
                return Err(unavailable(
                    "Terminal resize timed out; the session has been stopped.",
                ));
            }
        }
        self.snapshot(None)
    }

    fn send(&self, control: Control) -> Result<(), ApplicationError> {
        let state = self.shared.lock().expect("terminal output");
        if state.snapshot.status != TerminalSessionStatus::Running || state.child_done {
            return Err(unavailable("The terminal session has ended."));
        }
        drop(state);
        self.control
            .try_send(control)
            .map_err(|_| unavailable("Terminal is busy; try again."))
    }

    pub fn request_stop(&self) {
        let mut state = self.shared.lock().expect("terminal output");
        if state.stop_requested {
            return;
        }
        state.stop_requested = true;
        if state.snapshot.status == TerminalSessionStatus::Running {
            state.snapshot.status = if state.child_done {
                TerminalSessionStatus::Exited
            } else {
                TerminalSessionStatus::Stopped
            };
        }
        state.refresh_completion();
        drop(state);
        // Stop never competes with a saturated input queue: a dedicated sender
        // thread can block briefly while the IO worker drains rejected writes.
        let sender = self.control.clone();
        let ownership = self.ownership.clone();
        thread::spawn(move || {
            // Termination is independent of a possibly blocked PTY write.
            let _ = ownership
                .lock()
                .expect("terminal process ownership")
                .terminate();
            let _ = sender.send(Control::Stop);
        });
    }

    pub async fn stop(&self) -> Result<TerminalSessionSnapshot, ApplicationError> {
        self.request_stop();
        // Reap and drain within a bounded deadline. No OS IO runs on Tokio.
        let drained = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let notified = self.done.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                let finished = {
                    let state = self.shared.lock().expect("terminal output");
                    state.reader_done && state.child_done
                };
                if finished {
                    break;
                }
                notified.await;
            }
        })
        .await;
        if drained.is_err() {
            let mut state = self.shared.lock().expect("terminal output");
            if !state.child_done {
                return Err(unavailable(
                    "Terminal process has not stopped yet; try stopping it again shortly.",
                ));
            }
            // A descendant or unusual PTY driver can hold an output pipe open
            // beyond the reaping deadline. Make incomplete capture explicit.
            state.output.truncated = true;
        }
        self.snapshot(None)
    }
}

/// A shell spawn error still owns a live pseudoconsole. Its teardown needs the
/// same independent output drain and initial cursor reply as a started session.
/// Moving all handles into this worker lets the failed open return promptly.
pub(super) fn cleanup_failed_pair(
    pair: PtyPair,
    reader: Option<Box<dyn Read + Send>>,
    writer: Option<Box<dyn Write + Send>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let reader = reader.or_else(|| pair.master.try_clone_reader().ok());
        let writer = writer.or_else(|| pair.master.take_writer().ok());
        let drain = reader.map(|mut reader| {
            thread::spawn(move || {
                let _ = std::io::copy(&mut reader, &mut std::io::sink());
            })
        });
        #[cfg(windows)]
        let writer = writer.map(|mut writer| {
            let _ = writer.write_all(b"\x1b[1;1R");
            let _ = writer.flush();
            writer
        });
        drop(pair.slave);
        drop(writer);
        drop(pair.master);
        if let Some(drain) = drain {
            let _ = drain.join();
        }
    })
}

impl Drop for TerminalRuntime {
    fn drop(&mut self) {
        self.request_stop();
    }
}

fn control_loop(
    master: Box<dyn MasterPty + Send>,
    mut writer: Box<dyn Write + Send>,
    ownership: Arc<Mutex<ownership::Ownership>>,
    receiver: mpsc::Receiver<Control>,
    shared: Arc<Mutex<Shared>>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            Control::Write(data, reply) => {
                let writable = {
                    let state = shared.lock().expect("terminal output");
                    state.snapshot.status == TerminalSessionStatus::Running && !state.child_done
                };
                if !writable {
                    let _ = reply.send(Err(unavailable("The terminal session has ended.")));
                    continue;
                }
                let result = writer
                    .write_all(data.as_bytes())
                    .and_then(|_| writer.flush())
                    .map_err(|_| unavailable("Terminal input could not be written."));
                #[cfg(windows)]
                if result.is_ok() && initial_cursor_response(&data) {
                    shared
                        .lock()
                        .expect("terminal output")
                        .initial_cursor_answered = true;
                }
                let _ = reply.send(result);
            }
            Control::Resize(cols, rows, reply) => {
                let result = master
                    .resize(size(cols, rows))
                    .map_err(|_| unavailable("Terminal could not be resized."));
                if result.is_ok() {
                    let mut state = shared.lock().expect("terminal output");
                    state.snapshot.cols = cols;
                    state.snapshot.rows = rows;
                }
                let _ = reply.send(result);
            }
            Control::Stop => break,
        }
    }
    #[cfg(windows)]
    if !shared
        .lock()
        .expect("terminal output")
        .initial_cursor_answered
    {
        // portable-pty enables INHERIT_CURSOR. A disconnected client may never
        // answer its first DSR. Old Windows ClosePseudoConsole can deadlock in
        // that state; this fixed protocol reply is sent only during teardown.
        let _ = writer.write_all(b"\x1b[1;1R");
        let _ = writer.flush();
    }
    let _ = ownership
        .lock()
        .expect("terminal process ownership")
        .terminate();
    drop(writer);
    // Closing ConPTY also releases its output pipe and foreground processes.
    drop(master);
}

#[cfg(windows)]
fn initial_cursor_response(data: &str) -> bool {
    let Some(coords) = data
        .strip_prefix("\x1b[")
        .and_then(|value| value.strip_suffix('R'))
    else {
        return false;
    };
    let Some((row, col)) = coords.split_once(';') else {
        return false;
    };
    [row, col].into_iter().all(|coord| {
        (1..=5).contains(&coord.len()) && coord.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        cols,
        rows,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn default_shell() -> String {
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned())
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL")
            .ok()
            .filter(|value| Path::new(value).is_absolute())
            .unwrap_or_else(|| "/bin/sh".to_owned())
    }
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}
