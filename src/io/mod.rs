#![allow(missing_docs)]

use std::io;

use stream::Stream;

mod copy;
pub use self::copy::copy;

mod chain;
pub use self::chain::Chain;

pub trait ReadStream: Stream<Item=(), Error=io::Error> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>>;

    fn chain<R>(self, other: R) -> Chain<Self, R>
        where R: ReadStream,
              Self: Sized,
    {
        chain::new(self, other)
    }
}

pub trait WriteStream: Stream<Item=(), Error=io::Error> {
    fn write(&mut self, buf: &[u8]) -> io::Result<Option<usize>>;

    fn flush(&mut self) -> io::Result<bool>;
}

impl<R: ?Sized> ReadStream for Box<R> where R: ReadStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        (**self).read(buf)
    }
}

impl<W: ?Sized> WriteStream for Box<W> where W: WriteStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<Option<usize>> {
        (**self).write(buf)
    }

    fn flush(&mut self) -> io::Result<bool> {
        (**self).flush()
    }
}
