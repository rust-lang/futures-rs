//! Core I/O traits and combinators.

use std::io as std_io;

use futures_core::{Async, Poll, task};
pub use futures_io::{AsyncRead, AsyncWrite, IoVec};
use bytes::{Buf, BufMut};

pub mod io;
pub mod codec;

mod allow_std;
mod codecs;
mod copy;
mod flush;
mod framed;
mod framed_read;
mod framed_write;
mod length_delimited;
// Requires "BufRead"
// mod lines;
mod read;
mod read_exact;
mod read_to_end;
// TODO: resolve. Temporary disabled because it requires "BufRead",
// which does not have an async equivalent.
// mod read_until;
mod shutdown;
mod split;
mod window;
mod write_all;

use self::codec::{Decoder, Encoder, Framed};
use self::split::{ReadHalf, WriteHalf};

/// An extension trait which adds utility methods to `AsyncRead` types.
pub trait AsyncReadExt: AsyncRead {
    /// Pull some bytes from this source into the specified `Buf`, returning
    /// how many bytes were read.
    ///
    /// The `buf` provided will have bytes read into it and the internal cursor
    /// will be advanced if any bytes were read. Note that this method typically
    /// will not reallocate the buffer provided.
    fn read_buf<B: BufMut>(&mut self, cx: &mut task::Context, buf: &mut B)
        -> Poll<usize, std_io::Error>
        where Self: Sized,
    {
        if !buf.has_remaining_mut() {
            return Ok(Async::Ready(0));
        }

        unsafe {
            let n = {
                // The `IoVec` type can't have a 0-length size, so we create a bunch
                // of dummy versions on the stack with 1 length which we'll quickly
                // overwrite.
                let b1: &mut [u8] = &mut [0];
                let b2: &mut [u8] = &mut [0];
                let b3: &mut [u8] = &mut [0];
                let b4: &mut [u8] = &mut [0];
                let b5: &mut [u8] = &mut [0];
                let b6: &mut [u8] = &mut [0];
                let b7: &mut [u8] = &mut [0];
                let b8: &mut [u8] = &mut [0];
                let b9: &mut [u8] = &mut [0];
                let b10: &mut [u8] = &mut [0];
                let b11: &mut [u8] = &mut [0];
                let b12: &mut [u8] = &mut [0];
                let b13: &mut [u8] = &mut [0];
                let b14: &mut [u8] = &mut [0];
                let b15: &mut [u8] = &mut [0];
                let b16: &mut [u8] = &mut [0];
                let mut bufs: [&mut IoVec; 16] = [
                    b1.into(), b2.into(), b3.into(), b4.into(),
                    b5.into(), b6.into(), b7.into(), b8.into(),
                    b9.into(), b10.into(), b11.into(), b12.into(),
                    b13.into(), b14.into(), b15.into(), b16.into(),
                ];
                let n = buf.bytes_vec_mut(&mut bufs);
                try_ready!(self.poll_vectored_read(cx, &mut bufs[..n]))
            };

            buf.advance_mut(n);
            Ok(Async::Ready(n))
        }
    }

    /// Provides a `Stream` and `Sink` interface for reading and writing to this
    /// `Io` object, using `Decode` and `Encode` to read and write the raw data.
    ///
    /// Raw I/O objects work with byte sequences, but higher-level code usually
    /// wants to batch these into meaningful chunks, called "frames". This
    /// method layers framing on top of an I/O object, by using the `Codec`
    /// traits to handle encoding and decoding of messages frames. Note that
    /// the incoming and outgoing frame types may be distinct.
    ///
    /// This function returns a *single* object that is both `Stream` and
    /// `Sink`; grouping this into a single object is often useful for layering
    /// things like gzip or TLS, which require both read and write access to the
    /// underlying object.
    ///
    /// If you want to work more directly with the streams and sink, consider
    /// calling `split` on the `Framed` returned by this method, which will
    /// break them into separate objects, allowing them to interact more easily.
    fn framed<T: Encoder + Decoder>(self, codec: T) -> Framed<Self, T>
        where Self: AsyncWrite + Sized,
    {
        framed::framed(self, codec)
    }

    /// Helper method for splitting this read/write object into two halves.
    ///
    /// The two halves returned implement the `Read` and `Write` traits,
    /// respectively.
    fn split(self) -> (ReadHalf<Self>, WriteHalf<Self>)
        where Self: AsyncWrite + Sized,
    {
        split::split(self)
    }
}

impl<T: AsyncRead + ?Sized> AsyncReadExt for T {}

/// An extension trait which adds utility methods to `AsyncWrite` types.
pub trait AsyncWriteExt: AsyncWrite {
    /// Write a `Buf` into this value, returning how many bytes were written.
    ///
    /// Note that this method will advance the `buf` provided automatically by
    /// the number of bytes written.
    fn write_buf<B: Buf>(&mut self, cx: &mut task::Context, buf: &mut B)
        -> Poll<usize, std_io::Error>
        where Self: Sized,
    {
        if !buf.has_remaining() {
            return Ok(Async::Ready(0));
        }

        let n = {
            // The `IoVec` type can't have a zero-length size, so create a dummy
            // version from a 1-length slice which we'll overwrite with the
            // `bytes_vec` method.
            static DUMMY: &[u8] = &[0];
            let iovec = <&IoVec>::from(DUMMY);
            let mut bufs = [iovec; 64];
            let n = buf.bytes_vec(&mut bufs);
            try_ready!(self.poll_vectored_write(cx, &bufs[..n]))
        };
        buf.advance(n);
        Ok(Async::Ready(n))
    }
}

impl<T: AsyncWrite + ?Sized> AsyncWriteExt for T {}
