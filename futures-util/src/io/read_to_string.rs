use super::read_to_end::{read_to_end_internal, ReadBuf};
use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncRead;
use std::pin::Pin;
use std::string::String;
use std::{io, str};

/// Future for the [`read_to_string`](super::AsyncReadExt::read_to_string) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadToString<'a, R: ?Sized> {
    reader: &'a mut R,
    read_buf: ReadBuf<'a>,
    done: bool,
}

impl<R: ?Sized + Unpin> Unpin for ReadToString<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> ReadToString<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut String) -> Self {
        // Safety: we ensure that the string is truncated back to its original
        // length if what we read was not valid utf8.
        let bytes = unsafe { buf.as_mut_vec() };
        let read_buf = ReadBuf::new(bytes);
        Self { reader, read_buf, done: false }
    }
}

impl<R: ?Sized> Drop for ReadToString<'_, R> {
    fn drop(&mut self) {
        if !self.done {
            // If the future was canceled we cannot guarantee what we read so
            // far is valid utf-8. Restore the initial contents of the string.
            self.read_buf.reset();
        }
    }
}

impl<A> Future for ReadToString<'_, A>
where
    A: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, read_buf, done } = &mut *self;
        let ret = ready!(read_to_end_internal(Pin::new(reader), cx, read_buf));
        if str::from_utf8(read_buf.as_slice()).is_ok() {
            *done = true;
            Poll::Ready(ret)
        } else {
            Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            )))
        }
    }
}
