use std::io;

use io::{ReadStream, WriteStream};

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
