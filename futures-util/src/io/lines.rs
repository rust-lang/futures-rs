use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_io::AsyncBufRead;
use std::io;
use std::mem;
use std::pin::Pin;
use super::read_line::read_line_internal;
use pin_project::{pin_project, project};

/// Stream for the [`lines`](super::AsyncBufReadExt::lines) method.

#[pin_project]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Lines<R> {
    #[pin]
    reader: R,
    buf: String,
    bytes: Vec<u8>,
    read: usize,
}

impl<R: AsyncBufRead> Lines<R> {
    pub(super) fn new(reader: R) -> Self {
        Self {
            reader,
            buf: String::new(),
            bytes: Vec::new(),
            read: 0,
        }
    }
}

impl<R: AsyncBufRead> Stream for Lines<R> {
    type Item = io::Result<String>;

    #[project]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        #[project]
        let Lines { reader, buf, bytes, read } = self.project();
        let n = ready!(read_line_internal(reader, cx, buf, bytes, read))?;
        if n == 0 && buf.is_empty() {
            return Poll::Ready(None)
        }
        if buf.ends_with('\n') {
            buf.pop();
            if buf.ends_with('\r') {
                buf.pop();
            }
        }
        Poll::Ready(Some(Ok(mem::replace(buf, String::new()))))
    }
}
