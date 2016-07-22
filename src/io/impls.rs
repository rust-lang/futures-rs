use std::io;

use io::{ReadStream, WriteStream, BufReadStream};

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

impl<R: ?Sized> BufReadStream for Box<R> where R: BufReadStream {
    fn fill_buf(&mut self) -> io::Result<Option<&[u8]>> {
        (**self).fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        (**self).consume(amt)
    }
}
