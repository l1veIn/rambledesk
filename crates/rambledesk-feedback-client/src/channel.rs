//! Instance-owned local IPC. Only the controller holds the HTTP bearer; model
//! shell tools inherit a local channel address, never a credential-shaped env.
//! The address is private runtime metadata, not a persistent Agent setting.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rambledesk_core::ManagedFeedbackEndpoint;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::task::{JoinHandle, JoinSet};

use crate::{ClientError, MAX_INPUT_BYTES, MAX_RESPONSE_BYTES};

#[derive(Default, Clone, Copy)]
pub struct TurnReceipt {
    pub attempted: bool,
    pub handed_off: bool,
}

#[derive(Default)]
struct TurnState {
    generation: u64,
    receipt: TurnReceipt,
}

#[derive(Serialize, Deserialize)]
struct Request {
    operation: String,
    input: Value,
}

#[derive(Serialize, Deserialize)]
struct Response {
    success: bool,
    value: Value,
}

pub struct FeedbackChannel {
    address: String,
    receipt: Arc<Mutex<TurnState>>,
    task: Mutex<Option<JoinHandle<()>>>,
    #[cfg(unix)]
    _directory: tempfile::TempDir,
}

impl FeedbackChannel {
    pub fn start(endpoint: ManagedFeedbackEndpoint) -> Result<Self, ClientError> {
        crate::validate_endpoint(&endpoint)?;
        let receipt = Arc::new(Mutex::new(TurnState::default()));
        #[cfg(unix)]
        let directory = {
            use std::os::unix::fs::PermissionsExt;
            // A short path also works within macOS's 104-byte sockaddr_un limit.
            let directory = tempfile::Builder::new()
                .prefix("rd-fb-")
                .tempdir_in("/tmp")
                .map_err(|_| ClientError::RuntimeUnavailable)?;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .map_err(|_| ClientError::RuntimeUnavailable)?;
            directory
        };
        #[cfg(unix)]
        let address = directory
            .path()
            .join("feedback.sock")
            .to_string_lossy()
            .into_owned();
        #[cfg(unix)]
        let listener = tokio::net::UnixListener::bind(&address)
            .map_err(|_| ClientError::RuntimeUnavailable)?;
        #[cfg(windows)]
        let address = format!(r"\\.\pipe\rambledesk-feedback-{}", uuid::Uuid::now_v7());
        #[cfg(windows)]
        let listener = pipe(&address, true)?;
        let task_receipt = receipt.clone();
        #[cfg(windows)]
        let task_address = address.clone();
        let task = tokio::spawn(async move {
            let mut requests = JoinSet::new();
            #[cfg(windows)]
            let mut listener = listener;
            loop {
                // Drain completed requests so this instance retains no transcripts.
                while requests.try_join_next().is_some() {}
                #[cfg(unix)]
                let stream = match listener.accept().await {
                    Ok((stream, _)) => stream,
                    Err(_) => break,
                };
                #[cfg(windows)]
                let stream = {
                    if listener.connect().await.is_err() {
                        break;
                    }
                    let next = match pipe(&task_address, false) {
                        Ok(next) => next,
                        Err(_) => break,
                    };
                    std::mem::replace(&mut listener, next)
                };
                if requests.len() >= 8 {
                    drop(stream);
                    continue;
                }
                let endpoint = endpoint.clone();
                let receipt = task_receipt.clone();
                requests.spawn(async move {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(65),
                        serve(stream, endpoint, receipt),
                    )
                    .await;
                });
            }
            // Dropping JoinSet cancels unfinished local clients. The HTTP service
            // retains its own admitted-operation/revocation boundary.
        });
        Ok(Self {
            address,
            receipt,
            task: Mutex::new(Some(task)),
            #[cfg(unix)]
            _directory: directory,
        })
    }

    pub fn address(&self) -> &str {
        &self.address
    }
    pub fn begin_turn(&self) {
        let mut state = self.receipt.lock().expect("feedback receipt");
        state.generation = state.generation.wrapping_add(1);
        state.receipt = TurnReceipt::default();
    }
    pub fn receipt(&self) -> TurnReceipt {
        self.receipt.lock().expect("feedback receipt").receipt
    }
    pub async fn close(&self) {
        let task = self.task.lock().expect("feedback channel task").take();
        if let Some(task) = task {
            task.abort();
            let _ = task.await;
        }
    }
}

impl Drop for FeedbackChannel {
    fn drop(&mut self) {
        if let Some(task) = self.task.get_mut().expect("feedback channel task").take() {
            task.abort();
        }
    }
}

#[cfg(windows)]
fn pipe(
    address: &str,
    first: bool,
) -> Result<tokio::net::windows::named_pipe::NamedPipeServer, ClientError> {
    // Windows' default pipe DACL grants write access to the creator/administrators.
    // Reject remote clients, and refuse pre-existing names on initial creation.
    tokio::net::windows::named_pipe::ServerOptions::new()
        .first_pipe_instance(first)
        .reject_remote_clients(true)
        .create(address)
        .map_err(|_| ClientError::RuntimeUnavailable)
}

async fn read_frame<R: AsyncRead + Unpin>(
    stream: &mut R,
    limit: usize,
) -> Result<Vec<u8>, ClientError> {
    let length = stream
        .read_u32()
        .await
        .map_err(|_| ClientError::InvalidResponse)? as usize;
    if length > limit {
        return Err(ClientError::InvalidInput);
    }
    let mut bytes = vec![0; length];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|_| ClientError::InvalidResponse)?;
    Ok(bytes)
}

async fn write_frame<W: AsyncWrite + Unpin>(
    stream: &mut W,
    value: &impl Serialize,
) -> Result<(), ClientError> {
    let bytes = serde_json::to_vec(value).map_err(|_| ClientError::InvalidInput)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(ClientError::InvalidInput);
    }
    stream
        .write_u32(bytes.len() as u32)
        .await
        .map_err(|_| ClientError::UpstreamUnavailable)?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|_| ClientError::UpstreamUnavailable)?;
    stream
        .flush()
        .await
        .map_err(|_| ClientError::UpstreamUnavailable)
}

async fn serve<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    endpoint: ManagedFeedbackEndpoint,
    receipt: Arc<Mutex<TurnState>>,
) -> Result<(), ClientError> {
    let request: Request =
        serde_json::from_slice(&read_frame(&mut stream, MAX_INPUT_BYTES + 1024).await?)
            .map_err(|_| ClientError::InvalidInput)?;
    if !matches!(
        request.operation.as_str(),
        "request" | "get" | "recover" | "skip"
    ) {
        return Err(ClientError::InvalidInput);
    }
    let generation = {
        let mut state = receipt.lock().expect("feedback receipt");
        state.receipt.attempted = true;
        state.generation
    };
    let result = if request.operation == "skip" {
        match request.input["reason"].as_str() {
            Some("user_opt_out" | "task_finished" | "request_cancelled") => Ok((
                true,
                json!({"status":"skipped","reason":request.input["reason"]}),
            )),
            _ => Err(ClientError::InvalidInput),
        }
    } else {
        crate::call(&endpoint, &request.operation, &request.input).await
    };
    let (success, value) = match result {
        Ok(result) => result,
        Err(error) => (false, error.json(request.input["request_id"].as_str())),
    };
    if success
        && (request.operation == "skip"
            || request.operation == "request"
            || matches!(
                value["status"].as_str(),
                Some("waiting" | "in_progress" | "cancelled")
            )
            || matches!(value["resolution"].as_str(), Some("approved" | "cancelled")))
    {
        let mut state = receipt.lock().expect("feedback receipt");
        if state.generation == generation {
            state.receipt.handed_off = true;
        }
    }
    write_frame(&mut stream, &Response { success, value }).await
}

pub async fn call(
    address: &str,
    operation: &str,
    input: &Value,
) -> Result<(bool, Value), ClientError> {
    validate_address(address)?;
    #[cfg(unix)]
    let stream = {
        tokio::net::UnixStream::connect(address)
            .await
            .map_err(|_| ClientError::RevokedCapability)?
    };
    #[cfg(windows)]
    let stream = {
        let mut tries = 0;
        loop {
            match tokio::net::windows::named_pipe::ClientOptions::new().open(address) {
                Ok(stream) => break stream,
                Err(error) if error.raw_os_error() == Some(231) && tries < 20 => {
                    tries += 1;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(_) => return Err(ClientError::RevokedCapability),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(70), exchange(stream, operation, input))
        .await
        .map_err(|_| ClientError::UpstreamUnavailable)?
}

pub fn validate_address(address: &str) -> Result<(), ClientError> {
    #[cfg(unix)]
    let valid = {
        let path = std::path::Path::new(address);
        path.is_absolute()
            && path.file_name().and_then(|name| name.to_str()) == Some("feedback.sock")
    };
    #[cfg(windows)]
    let valid = address
        .strip_prefix(r"\\.\pipe\rambledesk-feedback-")
        .is_some_and(|suffix| uuid::Uuid::parse_str(suffix).is_ok());
    if valid {
        Ok(())
    } else {
        Err(ClientError::InvalidCapability)
    }
}

#[cfg(test)]
#[path = "channel_tests.rs"]
mod tests;

async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    operation: &str,
    input: &Value,
) -> Result<(bool, Value), ClientError> {
    write_frame(
        &mut stream,
        &Request {
            operation: operation.into(),
            input: input.clone(),
        },
    )
    .await?;
    let bytes = read_frame(&mut stream, MAX_RESPONSE_BYTES).await?;
    let response: Response =
        serde_json::from_slice(&bytes).map_err(|_| ClientError::InvalidResponse)?;
    Ok((response.success, response.value))
}
