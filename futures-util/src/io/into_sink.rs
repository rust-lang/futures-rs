use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use futures_sink::Sink;
use std::fmt;
use std::io;
use std::pin::Pin;
use std::marker::Unpin;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

struct Block {
    offset: usize,
    bytes: Box<dyn AsRef<[u8]>>,
}

/// Sink for the [`into_sink`](super::AsyncWriteExt::into_sink) method.
#[must_use = "sinks do nothing unless polled"]
#[derive(Debug)]
pub struct IntoSink<W> {
    writer: W,
    /// An outstanding block for us to push into the underlying writer, along with an offset of how
    /// far into this block we have written already.
    buffer: Option<Block>,
}

impl<W: Unpin> Unpin for IntoSink<W> {}

impl<W: AsyncWrite> IntoSink<W> {
    unsafe_pinned!(writer: W);
    unsafe_unpinned!(buffer: Option<Block>);

    pub(super) fn new(writer: W) -> Self {
        IntoSink { writer, buffer: None }
    }

    fn project<'a>(self: Pin<&'a mut Self>) -> (Pin<&'a mut W>, &'a mut Option<Block>) {
        unsafe {
            let this = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut this.writer), &mut this.buffer)
        }
    }

    /// If we have an outstanding block in `buffer` attempt to push it into the writer, does _not_
    /// flush the writer after it succeeds in pushing the block into it.
    fn poll_flush_buffer(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>>
    {
        let (mut writer, buffer) = self.project();
        if let Some(buffer) = buffer {
            loop {
                let bytes = (*buffer.bytes).as_ref();
                let written = ready!(writer.as_mut().poll_write(cx, &bytes[buffer.offset..]))?;
                buffer.offset += written;
                if buffer.offset == bytes.len() {
                    break;
                }
            }
        }
        *buffer = None;
        Poll::Ready(Ok(()))
    }

}

impl<W: AsyncWrite, Item: AsRef<[u8]> + 'static> Sink<Item> for IntoSink<W> {
    type SinkError = io::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>>
    {
        ready!(self.as_mut().poll_flush_buffer(cx))?;
        Poll::Ready(Ok(()))
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Item,
    ) -> Result<(), Self::SinkError>
    {
        debug_assert!(self.as_mut().buffer().is_none());
        *self.as_mut().buffer() = Some(Block { offset: 0, bytes: Box::new(item)} );
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>>
    {
        ready!(self.as_mut().poll_flush_buffer(cx))?;
        ready!(self.as_mut().writer().poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>>
    {
        ready!(self.as_mut().poll_flush_buffer(cx))?;
        ready!(self.as_mut().writer().poll_close(cx))?;
        Poll::Ready(Ok(()))
    }
}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[... {}/{} bytes ...]", self.offset, (*self.bytes).as_ref().len())?;
        Ok(())
    }
}
