use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_core::Stream;
use futures_io::AsyncWrite;
use std::io;
use std::pin::Pin;

/// Future for the [`copy_into`](super::StreamExt::copy_into) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct CopyInto<'a, S: Unpin, W: ?Sized + Unpin> {
    stream: S,
    read_done: bool,
    writer: &'a mut W,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Option<Vec<u8>>,
}

impl<S: Unpin, W: ?Sized + Unpin> Unpin for CopyInto<'_, S, W> {}

impl<'a, S: Unpin, W: ?Sized + Unpin> CopyInto<'a, S, W> {
    pub(super) fn new(stream: S, writer: &'a mut W) -> Self {
        CopyInto {
            stream,
            read_done: false,
            writer,
            amt: 0,
            pos: 0,
            cap: 0,
            buf: None,
        }
    }
}

impl<S, W> Future for CopyInto<'_, S, W>
    where S: Stream<Item = Vec<u8>> + Unpin,
          W: AsyncWrite + ?Sized + Unpin,
{
    type Output = io::Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            if this.pos == this.cap && !this.read_done {
                match ready!(Pin::new(&mut this.stream).poll_next(cx)) {
                    Some(buf) => {
                        this.pos = 0;
                        this.cap = buf.len();
                        this.buf = Some(buf);
                    }
                    None => this.read_done = true,
                }
            }

            // If our buffer has some data, let's write it out!
            if let Some(buf) = &this.buf {
                while this.pos < this.cap {
                    let i = ready!(Pin::new(&mut this.writer).poll_write(cx, &buf[this.pos..this.cap]))?;
                    if i == 0 {
                        return Poll::Ready(Err(io::ErrorKind::WriteZero.into()))
                    } else {
                        this.pos += i;
                        this.amt += i as u64;
                    }
                }
            }

            // If we've written al the data and we've seen EOF, flush out the
            // data and finish the transfer.
            // done with the entire transfer.
            if this.pos == this.cap && this.read_done {
                ready!(Pin::new(&mut this.writer).poll_flush(cx))?;
                return Poll::Ready(Ok(this.amt));
            }
        }
    }
}
