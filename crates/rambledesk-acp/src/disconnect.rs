use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

/// The SDK's application task may outlive its stream. Track transport EOF directly
/// so a dead backend is never mistaken for a reusable live connection.
pub(crate) struct DisconnectReader<R> {
    pub inner: R,
    pub closed: Arc<AtomicBool>,
}
impl<R: AsyncRead + Unpin> AsyncRead for DisconnectReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let remaining = buffer.remaining();
        let result = Pin::new(&mut self.inner).poll_read(cx, buffer);
        if matches!(result, Poll::Ready(Err(_)))
            || (matches!(result, Poll::Ready(Ok(())))
                && remaining > 0
                && buffer.filled().len() == before)
        {
            self.closed.store(true, Ordering::SeqCst);
        }
        result
    }
}
