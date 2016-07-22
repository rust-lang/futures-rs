#![allow(missing_docs)]

use std::io;

use stream::Stream;

mod copy;
pub use self::copy::copy;

mod chain;
mod read_to_end;
mod take;
mod buf_reader;
pub use self::buf_reader::BufReader;
pub use self::chain::Chain;
pub use self::read_to_end::ReadToEnd;
pub use self::take::Take;

mod impls;

pub trait ReadStream: Stream<Item=(), Error=io::Error> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>>;

    // TODO: default method for reading into a vector which pushes bytes?

    // TODO: is this the wrong type signature?
    fn read_to_end(self, buf: Vec<u8>) -> ReadToEnd<Self>
        where Self: Sized,
    {
        read_to_end::new(self, buf)
    }

    fn chain<R>(self, other: R) -> Chain<Self, R>
        where R: ReadStream,
              Self: Sized,
    {
        chain::new(self, other)
    }

    fn take(self, amt: u64) -> Take<Self>
        where Self: Sized,
    {
        take::new(self, amt)
    }
}

pub trait WriteStream: Stream<Item=(), Error=io::Error> {
    fn write(&mut self, buf: &[u8]) -> io::Result<Option<usize>>;

    fn flush(&mut self) -> io::Result<bool>;
}

pub trait BufReadStream: ReadStream {
    fn fill_buf(&mut self) -> io::Result<Option<&[u8]>>;
    fn consume(&mut self, amt: usize);
}
