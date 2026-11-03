use super::read_to_end::{read_to_end_internal, ReadBuf};
use futures_core::future::Future;
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
    // Tracks the number of bytes written by poll_read.
    // Between polls, buf.len() is kept at original_len (which is guaranteed to be valid utf-8),
    // while this field remembers how far we actually read so we can resume correctly.
    read_len: usize,
}

impl<R: ?Sized + Unpin> Unpin for ReadToString<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> ReadToString<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut String) -> Self {
        let bytes = unsafe { buf.as_mut_vec() };
        let original_len = bytes.len();
        let read_buf = ReadBuf::new(bytes);
        Self { reader, read_buf, read_len: original_len }
    }
}

impl<A> Future for ReadToString<'_, A>
where
    A: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, read_buf, read_len } = &mut *self;

        // Expose the bytes written in prior polls so read_to_end_internal
        // resumes from the right position rather than overwriting them.
        read_buf.set_len(*read_len);

        let ret = read_to_end_internal(Pin::new(reader), cx, read_buf);

        // Remember how many bytes were already read.
        *read_len = read_buf.len();

        // Reset to the original len that is guaranteed to contain a valid string, so that
        // we are always in a valid state between polls, even if this future is forgotten.
        read_buf.reset();

        match ret {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => {
                // Expose all read_len bytes to validate UTF-8.
                read_buf.set_len(*read_len);
                if str::from_utf8(read_buf.as_slice()).is_ok() {
                    Poll::Ready(res)
                } else {
                    read_buf.reset();
                    Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "stream did not contain valid UTF-8",
                    )))
                }
            }
        }
    }
}
