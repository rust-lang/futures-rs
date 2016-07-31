use std::io;

use futures::Task;
use futures::stream::Stream;

use {ReadTask, WriteTask, Ready};

impl<R: ?Sized> ReadTask for R
    where R: io::Read + Stream<Item=Ready, Error=io::Error>,
{
    fn read(&mut self, _task: &mut Task, buf: &mut [u8]) -> io::Result<usize> {
        io::Read::read(self, buf)
    }

    fn read_to_end(&mut self,
                   _task: &mut Task,
                   buf: &mut Vec<u8>) -> io::Result<usize> {
        io::Read::read_to_end(self, buf)
    }
}

impl<W: ?Sized> WriteTask for W
    where W: io::Write + Stream<Item=Ready, Error=io::Error>,
{
    fn write(&mut self, _task: &mut Task, buf: &[u8]) -> io::Result<usize> {
        io::Write::write(self, buf)
    }

    fn flush(&mut self, _task: &mut Task) -> io::Result<()> {
        io::Write::flush(self)
    }
}
