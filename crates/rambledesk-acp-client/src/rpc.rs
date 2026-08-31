use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{Mutex, mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    AcpClientError, AcpErrorCode,
    process::{AgentReader, AgentWriter, ProcessControl, SpawnedAgent},
};

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const MAX_PENDING_REQUESTS: usize = 256;
const MAX_EXPIRED_REQUESTS: usize = MAX_PENDING_REQUESTS * 2;

#[derive(Debug)]
pub(crate) enum InboundMessage {
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    Disconnected {
        reason: String,
    },
}

pub(crate) struct RpcPeer {
    writer: Mutex<AgentWriter>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, AcpClientError>>>>,
    expired: Mutex<VecDeque<u64>>,
    next_id: AtomicU64,
    closed: AtomicBool,
    control: Arc<dyn ProcessControl>,
    reader_task: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl RpcPeer {
    pub(crate) fn start(agent: SpawnedAgent) -> (Arc<Self>, mpsc::Receiver<InboundMessage>) {
        let (inbound_tx, inbound_rx) = mpsc::channel(256);
        let peer = Arc::new(Self {
            writer: Mutex::new(agent.writer),
            pending: Mutex::new(HashMap::new()),
            expired: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            control: agent.control,
            reader_task: std::sync::Mutex::new(None),
        });
        let task_peer = peer.clone();
        let task = tokio::spawn(async move {
            task_peer.read_loop(agent.reader, inbound_tx).await;
        });
        *peer.reader_task.lock().expect("reader task lock poisoned") = Some(task);
        (peer, inbound_rx)
    }

    pub(crate) async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
    ) -> Result<Value, AcpClientError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(AcpClientError::disconnected("ACP connection is closed"));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            if pending.len() >= MAX_PENDING_REQUESTS {
                return Err(AcpClientError::new(
                    AcpErrorCode::RpcError,
                    "too many pending ACP requests",
                    true,
                ));
            }
            pending.insert(id, sender);
        }
        if let Err(error) = self
            .write(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params
            }))
            .await
        {
            self.pending.lock().await.remove(&id);
            return Err(error);
        }
        let result = if let Some(timeout) = timeout {
            match tokio::time::timeout(timeout, receiver).await {
                Ok(result) => result,
                Err(_) => {
                    if self.pending.lock().await.remove(&id).is_some() {
                        self.remember_expired(id).await;
                    }
                    return Err(AcpClientError::new(
                        AcpErrorCode::OperationTimedOut,
                        format!("ACP request {method} timed out"),
                        true,
                    ));
                }
            }
        } else {
            receiver.await
        };
        result.map_err(|_| AcpClientError::disconnected("ACP response channel closed"))?
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) async fn notify(&self, method: &str, params: Value) -> Result<(), AcpClientError> {
        self.write(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }))
        .await
    }

    pub(crate) async fn respond_result(
        &self,
        id: Value,
        result: Value,
    ) -> Result<(), AcpClientError> {
        self.write(&json!({"jsonrpc": "2.0", "id": id, "result": result}))
            .await
    }

    pub(crate) async fn respond_error(
        &self,
        id: Value,
        code: i64,
        message: &str,
    ) -> Result<(), AcpClientError> {
        self.write(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": code, "message": message}
        }))
        .await
    }

    pub(crate) async fn shutdown(&self, grace: Duration) -> Result<bool, AcpClientError> {
        self.closed.store(true, Ordering::Release);
        let _ = self.writer.lock().await.shutdown().await;
        let forced = self.control.shutdown(grace).await?;
        if let Some(task) = self
            .reader_task
            .lock()
            .expect("reader task lock poisoned")
            .take()
        {
            task.abort();
        }
        self.fail_pending("ACP connection shut down").await;
        Ok(forced)
    }

    async fn write(&self, value: &Value) -> Result<(), AcpClientError> {
        let mut encoded = serde_json::to_vec(value).map_err(|error| {
            AcpClientError::protocol(format!("could not serialize ACP frame: {error}"))
        })?;
        if encoded.len() > MAX_FRAME_BYTES {
            return Err(AcpClientError::invalid("ACP frame exceeds size limit"));
        }
        encoded.push(b'\n');
        let mut writer = self.writer.lock().await;
        writer.write_all(&encoded).await.map_err(|error| {
            AcpClientError::disconnected(format!("could not write ACP frame: {error}"))
        })?;
        writer.flush().await.map_err(|error| {
            AcpClientError::disconnected(format!("could not flush ACP frame: {error}"))
        })
    }

    async fn read_loop(
        self: Arc<Self>,
        reader: AgentReader,
        inbound: mpsc::Sender<InboundMessage>,
    ) {
        let mut reader = BufReader::new(reader);
        let disconnect_reason = loop {
            let line = match read_bounded_line(&mut reader).await {
                Ok(Some(line)) => line,
                Ok(None) => break "ACP Agent closed stdout".to_string(),
                Err(error) => break error,
            };
            let message: Value = match serde_json::from_slice(&line) {
                Ok(value) => value,
                Err(error) => break format!("invalid ACP JSON frame: {error}"),
            };
            match self.route(message, &inbound).await {
                Ok(()) => {}
                Err(error) => break error.message,
            }
        };
        self.closed.store(true, Ordering::Release);
        self.fail_pending(&disconnect_reason).await;
        let _ = inbound
            .send(InboundMessage::Disconnected {
                reason: disconnect_reason,
            })
            .await;
    }

    async fn route(
        &self,
        message: Value,
        inbound: &mpsc::Sender<InboundMessage>,
    ) -> Result<(), AcpClientError> {
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(AcpClientError::protocol(
                "ACP frame omitted jsonrpc 2.0 marker",
            ));
        }
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let routed = if let Some(id) = message.get("id") {
                InboundMessage::Request {
                    id: id.clone(),
                    method: method.to_string(),
                    params,
                }
            } else {
                InboundMessage::Notification {
                    method: method.to_string(),
                    params,
                }
            };
            inbound
                .send(routed)
                .await
                .map_err(|_| AcpClientError::disconnected("ACP inbound handler stopped"))?;
            return Ok(());
        }
        let id = message
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| AcpClientError::protocol("ACP response has an unknown request id"))?;
        let Some(responder) = self.pending.lock().await.remove(&id) else {
            if self.take_expired(id).await {
                return Ok(());
            }
            return Err(AcpClientError::protocol(format!(
                "ACP response id {id} was not pending"
            )));
        };
        let result = if let Some(error) = message.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(-32603);
            let detail = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("ACP request failed");
            let normalized = detail.to_ascii_lowercase();
            let error_code = if normalized.contains("authentication required")
                || normalized.contains("not authenticated")
                || normalized.contains("please log in")
                || normalized.contains("please login")
                || normalized.contains("sign in")
                || normalized.contains("valid license")
            {
                AcpErrorCode::AuthenticationRequired
            } else {
                AcpErrorCode::RpcError
            };
            let data = (code == -32602)
                .then(|| error.get("data"))
                .flatten()
                .filter(|value| !value.is_null())
                .map(|value| {
                    let mut encoded = value.to_string();
                    encoded.truncate(1_000);
                    format!(" ({encoded})")
                })
                .unwrap_or_default();
            Err(AcpClientError::new(
                error_code,
                format!("ACP error {code}: {detail}{data}"),
                error_code == AcpErrorCode::AuthenticationRequired
                    || code == -32603
                    || code == -32000,
            ))
        } else {
            Ok(message.get("result").cloned().unwrap_or(Value::Null))
        };
        let _ = responder.send(result);
        Ok(())
    }

    async fn fail_pending(&self, reason: &str) {
        let pending = std::mem::take(&mut *self.pending.lock().await);
        for (_, responder) in pending {
            let _ = responder.send(Err(AcpClientError::disconnected(reason)));
        }
    }

    async fn remember_expired(&self, id: u64) {
        let mut expired = self.expired.lock().await;
        expired.push_back(id);
        while expired.len() > MAX_EXPIRED_REQUESTS {
            expired.pop_front();
        }
    }

    async fn take_expired(&self, id: u64) -> bool {
        let mut expired = self.expired.lock().await;
        let Some(position) = expired.iter().position(|candidate| *candidate == id) else {
            return false;
        };
        expired.remove(position);
        true
    }
}

async fn read_bounded_line(reader: &mut BufReader<AgentReader>) -> Result<Option<Vec<u8>>, String> {
    let mut output = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|error| format!("could not read ACP frame: {error}"))?;
        if available.is_empty() {
            return if output.is_empty() {
                Ok(None)
            } else {
                Err("ACP Agent closed stdout mid-frame".to_string())
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let count = newline.map_or(available.len(), |index| index + 1);
        if output.len().saturating_add(count) > MAX_FRAME_BYTES {
            return Err("ACP frame exceeds size limit".to_string());
        }
        output.extend_from_slice(&available[..count]);
        reader.consume(count);
        if newline.is_some() {
            while output
                .last()
                .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
            {
                output.pop();
            }
            if output.is_empty() {
                continue;
            }
            return Ok(Some(output));
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    use super::*;
    use crate::process::{ProcessControl, SpawnedAgent};

    struct FakeControl;

    #[async_trait]
    impl ProcessControl for FakeControl {
        async fn shutdown(&self, _grace: Duration) -> Result<bool, AcpClientError> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn timeouts_release_pending_capacity_and_late_responses_are_ignored() {
        let (client, agent) = tokio::io::duplex(1024 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client);
        let (agent_reader, mut agent_writer) = tokio::io::split(agent);
        let (release_late_responses, wait_for_release) = oneshot::channel();
        tokio::spawn(async move {
            let mut lines = BufReader::new(agent_reader).lines();
            let mut expired_ids = Vec::new();
            for _ in 0..300 {
                let line = lines.next_line().await.unwrap().unwrap();
                let frame: Value = serde_json::from_str(&line).unwrap();
                expired_ids.push(frame["id"].as_u64().unwrap());
            }
            wait_for_release.await.unwrap();
            for id in expired_ids {
                agent_writer
                    .write_all(
                        format!("{}\n", json!({"jsonrpc":"2.0","id":id,"result":{}})).as_bytes(),
                    )
                    .await
                    .unwrap();
            }
            agent_writer.flush().await.unwrap();
            let line = lines.next_line().await.unwrap().unwrap();
            let frame: Value = serde_json::from_str(&line).unwrap();
            agent_writer
                .write_all(
                    format!(
                        "{}\n",
                        json!({"jsonrpc":"2.0","id":frame["id"],"result":{"ok":true}})
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            agent_writer.flush().await.unwrap();
        });
        let (peer, _inbound) = RpcPeer::start(SpawnedAgent {
            reader: Box::pin(client_reader),
            writer: Box::pin(client_writer),
            control: Arc::new(FakeControl),
        });

        for _ in 0..300 {
            let error = peer
                .request("slow", json!({}), Some(Duration::from_millis(1)))
                .await
                .expect_err("request should time out");
            assert_eq!(error.code, AcpErrorCode::OperationTimedOut);
        }
        release_late_responses.send(()).unwrap();
        let result = peer
            .request("healthy", json!({}), Some(Duration::from_secs(1)))
            .await
            .expect("connection remains usable after late responses");
        assert_eq!(result, json!({"ok":true}));
        peer.shutdown(Duration::from_millis(10)).await.unwrap();
    }
}
