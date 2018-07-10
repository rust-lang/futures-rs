//! IO
//!
//! This module contains a number of functions for working with
//! `AsyncRead` and `AsyncWrite` types, including the
//! `AsyncReadExt` and `AsyncWriteExt` traits which add methods
//! to the `AsyncRead` and `AsyncWrite` types.

use std::vec::Vec;

pub use futures_io::{AsyncRead, AsyncWrite, IoVec};

// Temporarily removed until AsyncBufRead is implemented
// pub use io::lines::{lines, Lines};
// pub use io::read_until::{read_until, ReadUntil};
// mod lines;
// mod read_until;

mod allow_std;
pub use self::allow_std::AllowStdIo;

mod copy_into;
pub use self::copy_into::CopyInto;

mod flush;
pub use self::flush::Flush;

mod read;
pub use self::read::Read;

mod read_exact;
pub use self::read_exact::ReadExact;

mod read_to_end;
pub use self::read_to_end::ReadToEnd;

mod close;
pub use self::close::Close;

mod split;
pub use self::split::{ReadHalf, WriteHalf};

mod window;
pub use self::window::Window;

mod write_all;
pub use self::write_all::WriteAll;

/// An extension trait which adds utility methods to `AsyncRead` types.
pub trait AsyncReadExt: AsyncRead {
    /// Creates a future which copies all the bytes from one object to another.
    ///
    /// The returned future will copy all the bytes read from this `AsyncRead` into the
    /// `writer` specified. This future will only complete once the `reader` has hit
    /// EOF and all bytes have been written to and flushed from the `writer`
    /// provided.
    ///
    /// On success the number of bytes is returned.
    fn copy_into<'a, W>(
        &'a mut self,
        writer: &'a mut W,
    ) -> CopyInto<'a, Self, W>
        where W: AsyncWrite,
    {
        CopyInto::new(self, writer)
    }

    /// Tries to read some bytes directly into the given `buf` in asynchronous
    /// manner, returning a future type.
    ///
    /// The returned future will resolve to the number of bytes read once the read
    /// operation is completed.
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Read<'a, Self> {
        Read::new(self, buf)
    }

    /// Creates a future which will read exactly enough bytes to fill `buf`,
    /// returning an error if EOF is hit sooner.
    ///
    /// The returned future will resolve once the read operation is completed.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded.
    fn read_exact<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> ReadExact<'a, Self> {
        ReadExact::new(self, buf)
    }

    /// Creates a future which will read all the bytes from this `AsyncRead`.
    fn read_to_end<'a>(
        &'a mut self,
        buf: &'a mut Vec<u8>,
    ) -> ReadToEnd<'a, Self> {
        ReadToEnd::new(self, buf)
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

impl<R: AsyncRead + ?Sized> AsyncReadExt for R {}

/// An extension trait which adds utility methods to `AsyncWrite` types.
pub trait AsyncWriteExt: AsyncWrite {
    /// Creates a future which will entirely flush this `AsyncWrite`.
    fn flush<'a>(&'a mut self) -> Flush<'a, Self> {
        Flush::new(self)
    }

    /// Creates a future which will entirely close this `AsyncWrite`.
    fn close<'a>(&'a mut self) -> Close<'a, Self> {
        Close::new(self)
    }

    /// Write data into this object.
    ///
    /// Creates a future that will write the entire contents of the buffer `buf` into
    /// this `AsyncWrite`.
    ///
    /// The returned future will not complete until all the data has been written.
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteAll<'a, Self> {
        WriteAll::new(self, buf)
    }
}

impl<W: AsyncWrite + ?Sized> AsyncWriteExt for W {}
