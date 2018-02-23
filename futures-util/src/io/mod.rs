//! Core I/O traits and combinators.

use std::io as std_io;
use std::vec::Vec;

use futures_core::{Async, Poll, task};
pub use futures_io::{AsyncRead, AsyncWrite, IoVec};
use bytes::{Buf, BufMut};

pub mod codec;

pub use self::allow_std::AllowStdIo;
pub use self::copy_into::CopyInto;
pub use self::flush::Flush;
pub use self::read::Read;
pub use self::read_exact::ReadExact;
pub use self::read_to_end::ReadToEnd;
pub use self::close::Close;
pub use self::split::{ReadHalf, WriteHalf};
pub use self::window::Window;
pub use self::write_all::WriteAll;

// Temporarily removed until AsyncBufRead is implemented
// pub use io::lines::{lines, Lines};
// pub use io::read_until::{read_until, ReadUntil};
// mod lines;
// mod read_until;

mod allow_std;
mod codecs;
mod copy_into;
mod flush;
mod framed;
mod framed_read;
mod framed_write;
mod length_delimited;
mod read;
mod read_exact;
mod read_to_end;
mod close;
mod split;
mod window;
mod write_all;

use self::codec::{Decoder, Encoder, Framed};

/// An extension trait which adds utility methods to `AsyncRead` types.
pub trait AsyncReadExt: AsyncRead {
    /// Pull some bytes from this source into the specified `Buf`, returning
    /// how many bytes were read.
    ///
    /// The `buf` provided will have bytes read into it and the internal cursor
    /// will be advanced if any bytes were read. Note that this method typically
    /// will not reallocate the buffer provided.
    fn poll_read_buf<B: BufMut>(&mut self, cx: &mut task::Context, buf: &mut B)
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

    /// Creates a future which copies all the bytes from one object to another.
    ///
    /// The returned future will copy all the bytes read from this `AsyncRead` into the
    /// `writer` specified. This future will only complete once the `reader` has hit
    /// EOF and all bytes have been written to and flushed from the `writer`
    /// provided.
    ///
    /// On success the number of bytes is returned and this `AsyncRead` and `writer` are
    /// consumed. On error the error is returned and the I/O objects are consumed as
    /// well.
    fn copy_into<W>(self, writer: W) -> CopyInto<Self, W>
        where W: AsyncWrite,
              Self: Sized,
    {
        copy_into::copy_into(self, writer)
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

    /// Tries to read some bytes directly into the given `buf` in asynchronous
    /// manner, returning a future type.
    ///
    /// The returned future will resolve to both the I/O stream and the buffer
    /// as well as the number of bytes read once the read operation is completed.
    fn read<T>(self, buf: T) -> Read<Self, T>
        where T: AsMut<[u8]>,
              Self: Sized,
    {
        read::read(self, buf)
    }


    /// Creates a future which will read exactly enough bytes to fill `buf`,
    /// returning an error if EOF is hit sooner.
    ///
    /// The returned future will resolve to both the I/O stream as well as the
    /// buffer once the read operation is completed.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded. In the case of success the object will be destroyed and
    /// the buffer will be returned, with all data read from the stream appended to
    /// the buffer.
    fn read_exact<T>(self, buf: T) -> ReadExact<Self, T>
        where T: AsMut<[u8]>,
              Self: Sized,
    {
        read_exact::read_exact(self, buf)
    }

    /// Creates a future which will read all the bytes from this `AsyncRead`.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded. In the case of success the object will be destroyed and
    /// the buffer will be returned, with all data read from the stream appended to
    /// the buffer.
    fn read_to_end(self, buf: Vec<u8>) -> ReadToEnd<Self>
        where Self: Sized,
    {
        read_to_end::read_to_end(self, buf)
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
    /// Creates a future which will entirely flush this `AsyncWrite` and then return `self`.
    ///
    /// This function will consume `self` if an error occurs.
    fn flush(self) -> Flush<Self>
        where Self: Sized,
    {
        flush::flush(self)
    }

    /// Creates a future which will entirely close this `AsyncWrite` and then return `self`.
    ///
    /// This function will consume the object provided if an error occurs.
    fn close(self) -> Close<Self>
        where Self: Sized,
    {
        close::close(self)
    }

    /// Write a `Buf` into this value, returning how many bytes were written.
    ///
    /// Note that this method will advance the `buf` provided automatically by
    /// the number of bytes written.
    fn poll_write_buf<B: Buf>(&mut self, cx: &mut task::Context, buf: &mut B)
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

    /// Creates a future that will write the entire contents of the buffer `buf` into
    /// this `AsyncWrite`.
    ///
    /// The returned future will not complete until all the data has been written.
    /// The future will resolve to a tuple of `self` and `buf`
    /// (so the buffer can be reused as needed).
    ///
    /// Any error which happens during writing will cause both the stream and the
    /// buffer to be destroyed.
    ///
    /// The `buf` parameter here only requires the `AsRef<[u8]>` trait, which should
    /// be broadly applicable to accepting data which can be converted to a slice.
    /// The `Window` struct is also available in this crate to provide a different
    /// window into a slice if necessary.
    fn write_all<T>(self, buf: T) -> WriteAll<Self, T>
        where T: AsRef<[u8]>,
              Self: Sized,
    {
        write_all::write_all(self, buf)
    }
}

impl<T: AsyncWrite + ?Sized> AsyncWriteExt for T {}
