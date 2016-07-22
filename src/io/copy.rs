use std::io;

use {Future, Poll, Task};
use io::{ReadStream, WriteStream};

pub struct Copy<R, W> {
    reader: R,
    read_ready: bool,
    writer: W,
    write_ready: bool,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: [u8; 2048],
}

pub fn copy<R, W>(reader: R, writer: W) -> Copy<R, W>
    where R: ReadStream,
          W: WriteStream,
{
    Copy {
        reader: reader,
        read_ready: true,
        writer: writer,
        write_ready: true,
        amt: 0,
        pos: 0,
        cap: 0,
        buf: [0; 2048],
    }
}

impl<R, W> Future for Copy<R, W>
    where R: ReadStream,
          W: WriteStream,
{
    type Item = u64;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<u64, io::Error> {
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            if self.pos == self.cap {
                if !self.read_ready {
                    match try_poll!(self.reader.poll(task)) {
                        Ok(_) => self.read_ready = true,
                        Err(e) => return Poll::Err(e),
                    }
                }
                match self.reader.read(&mut self.buf) {
                    Ok(Some(0)) => return Poll::Ok(self.amt),
                    Ok(Some(i)) => {
                        self.pos = 0;
                        self.cap = i;
                    }
                    Ok(None) => {
                        self.read_ready = false;
                        return Poll::NotReady
                    }
                    Err(e) => return Poll::Err(e),
                }
            }

            // Now that our buffer has some data, let's write it out!
            while self.pos < self.cap {
                if !self.write_ready {
                    match try_poll!(self.writer.poll(task)) {
                        Ok(_) => self.write_ready = true,
                        Err(e) => return Poll::Err(e),
                    }
                }
                match self.writer.write(&self.buf[self.pos..self.cap]) {
                    Ok(Some(i)) => {
                        self.pos += i;
                        self.amt += i as u64;
                    }
                    Ok(None) => {
                        self.write_ready = false;
                        return Poll::NotReady
                    }
                    Err(e) => return Poll::Err(e),
                }
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if self.read_ready && self.write_ready {
            task.notify();
        }
        if !self.read_ready {
            self.reader.schedule(task);
        }
        if !self.write_ready {
            self.writer.schedule(task);
        }
    }
}
