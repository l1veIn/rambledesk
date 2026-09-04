use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

/// rmcp buffers a newline-delimited message. Cap a partial line too, so a
/// malfunctioning peer cannot grow that buffer indefinitely without a newline.
pub(super) struct BoundedInput<R> {
    reader: R,
    line_bytes: usize,
    maximum: usize,
}
impl<R> BoundedInput<R> {
    pub fn new(reader: R, maximum: usize) -> Self {
        Self {
            reader,
            line_bytes: 0,
            maximum,
        }
    }
}
impl<R: AsyncRead + Unpin> AsyncRead for BoundedInput<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        match Pin::new(&mut self.reader).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                for byte in &buf.filled()[before..] {
                    if *byte == b'\n' {
                        self.line_bytes = 0;
                    } else {
                        self.line_bytes += 1;
                        if self.line_bytes > self.maximum {
                            // AsyncRead requires an error to leave the caller's
                            // filled buffer unchanged, even though the peer's
                            // offending bytes have been consumed internally.
                            buf.set_filled(before);
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "Managed MCP frame exceeds the input limit",
                            )));
                        }
                    }
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
