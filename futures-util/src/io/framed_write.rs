use std::io::{self, Read};
use std::fmt;

use futures_io::{AsyncRead, AsyncWrite, Initializer, IoVec};
use io::codec::Decoder;
use io::framed::Fuse;

use {Async, Poll, Stream, Sink, task};
use bytes::BytesMut;

/// Trait of helper objects to write out messages as bytes, for use with
/// `FramedWrite`.
pub trait Encoder {
    /// The type of items consumed by the `Encoder`
    type Item;

    /// The type of encoding errors.
    ///
    /// `FramedWrite` requires `Encoder`s errors to implement `From<io::Error>`
    /// in the interest letting it return `Error`s directly.
    type Error: From<io::Error>;

    /// Encodes a frame into the buffer provided.
    ///
    /// This method will encode `item` into the byte buffer provided by `dst`.
    /// The `dst` provided is an internal buffer of the `Framed` instance and
    /// will be written out when possible.
    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut)
              -> Result<(), Self::Error>;
}

/// A `Sink` of frames encoded to an `AsyncWrite`.
pub struct FramedWrite<T, E> {
    inner: FramedWrite2<Fuse<T, E>>,
}

pub struct FramedWrite2<T> {
    inner: T,
    buffer: BytesMut,
}

const INITIAL_CAPACITY: usize = 8 * 1024;
const BACKPRESSURE_BOUNDARY: usize = INITIAL_CAPACITY;

impl<T, E> FramedWrite<T, E>
    where T: AsyncWrite,
          E: Encoder,
{
    /// Creates a new `FramedWrite` with the given `encoder`.
    pub fn new(inner: T, encoder: E) -> FramedWrite<T, E> {
        FramedWrite {
            inner: framed_write2(Fuse(inner, encoder)),
        }
    }
}

impl<T, E> FramedWrite<T, E> {
    /// Returns a reference to the underlying I/O stream wrapped by
    /// `FramedWrite`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn get_ref(&self) -> &T {
        &self.inner.inner.0
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by
    /// `FramedWrite`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner.inner.0
    }

    /// Consumes the `FramedWrite`, returning its underlying I/O stream.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn into_inner(self) -> T {
        self.inner.inner.0
    }

    /// Returns a reference to the underlying decoder.
    pub fn encoder(&self) -> &E {
        &self.inner.inner.1
    }

    /// Returns a mutable reference to the underlying decoder.
    pub fn encoder_mut(&mut self) -> &mut E {
        &mut self.inner.inner.1
    }
}

impl<T, E> Sink for FramedWrite<T, E>
    where T: AsyncWrite,
          E: Encoder,
{
    type SinkItem = E::Item;
    type SinkError = E::Error;

    delegate_sink!(inner);
}

impl<T, D> Stream for FramedWrite<T, D>
    where T: Stream,
{
    type Item = T::Item;
    type Error = T::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        self.inner.inner.0.poll_next(cx)
    }
}

impl<T, U> fmt::Debug for FramedWrite<T, U>
    where T: fmt::Debug,
          U: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FramedWrite")
         .field("inner", &self.inner.get_ref().0)
         .field("encoder", &self.inner.get_ref().1)
         .field("buffer", &self.inner.buffer)
         .finish()
    }
}

// ===== impl FramedWrite2 =====

pub fn framed_write2<T>(inner: T) -> FramedWrite2<T> {
    FramedWrite2 {
        inner,
        buffer: BytesMut::with_capacity(INITIAL_CAPACITY),
    }
}

pub fn framed_write2_with_buffer<T>(inner: T, mut buf: BytesMut) -> FramedWrite2<T> {
    if buf.capacity() < INITIAL_CAPACITY {
        let bytes_to_reserve = INITIAL_CAPACITY - buf.capacity();
        buf.reserve(bytes_to_reserve);
    }
    FramedWrite2 {
        inner,
        buffer: buf,
    }
}

impl<T> FramedWrite2<T> {
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    pub fn into_parts(self) -> (T, BytesMut) {
        (self.inner, self.buffer)
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: AsyncWrite + Encoder> FramedWrite2<T> {
    fn poll_write_internal(&mut self, cx: &mut task::Context) -> Poll<(), T::Error> {
        trace!("flushing framed transport");

        while !self.buffer.is_empty() {
            trace!("writing; remaining={}", self.buffer.len());

            let n = try_ready!(self.inner.poll_write(cx, &self.buffer));

            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::WriteZero, "failed to
                                          write frame to transport").into());
            }

            // TODO: Add a way to `bytes` to do this w/o returning the drained
            // data.
            let _ = self.buffer.split_to(n);
        }
        Ok(Async::Ready(()))
    }
}

impl<T> Sink for FramedWrite2<T>
    where T: AsyncWrite + Encoder,
{
    type SinkItem = T::Item;
    type SinkError = T::Error;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        // If the buffer is already over 8KiB, then attempt to flush it. If after flushing it's
        // *still* over 8KiB, then apply backpressure (reject the send).
        if self.buffer.len() < BACKPRESSURE_BOUNDARY {
            return Ok(Async::Ready(()));
        }

        self.poll_flush(cx)?;

        if self.buffer.len() >= BACKPRESSURE_BOUNDARY {
            Ok(Async::Pending)
        } else {
            Ok(Async::Ready(()))
        }
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        self.inner.encode(item, &mut self.buffer)
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        try_ready!(self.poll_write_internal(cx));
        self.inner.poll_flush(cx).map_err(Into::into)
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        try_ready!(self.poll_write_internal(cx));
        self.inner.poll_close(cx).map_err(Into::into)
    }
}

impl<T: Decoder> Decoder for FramedWrite2<T> {
    type Item = T::Item;
    type Error = T::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<T::Item>, T::Error> {
        self.inner.decode(src)
    }

    fn decode_eof(&mut self, src: &mut BytesMut) -> Result<Option<T::Item>, T::Error> {
        self.inner.decode_eof(src)
    }
}

impl<T: Read> Read for FramedWrite2<T> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.inner.read(dst)
    }
}

impl<T: AsyncRead> AsyncRead for FramedWrite2<T> {
    unsafe fn initializer(&self) -> Initializer {
        self.inner.initializer()
    }

    fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8])
        -> Poll<usize, io::Error>
    {
        self.inner.poll_read(cx, buf)
    }

    fn poll_vectored_read(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVec])
        -> Poll<usize, io::Error>
    {
        self.inner.poll_vectored_read(cx, vec)
    }
}
