use std::io;

use {Future, Poll, Task};
use io::{ReadTask, WriteTask};

pub struct Copy<R, W> {
    reader: R,
    read_ready: bool,
    read_done: bool,
    writer: W,
    write_ready: bool,
    flush_done: bool,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: [u8; 2048],
}

pub fn copy<R, W>(reader: R, writer: W) -> Copy<R, W>
    where R: ReadTask,
          W: WriteTask,
{
    Copy {
        reader: reader,
        read_ready: true,
        read_done: false,
        writer: writer,
        write_ready: true,
        flush_done: false,
        amt: 0,
        pos: 0,
        cap: 0,
        buf: [0; 2048],
    }
}

impl<R, W> Future for Copy<R, W>
    where R: ReadTask,
          W: WriteTask,
{
    type Item = u64;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<u64, io::Error> {
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            if !self.read_done && self.pos == self.cap {
                if !self.read_ready {
                    match try_poll!(self.reader.poll(task)) {
                        Ok(Some(ref r)) if r.is_read() => self.read_ready = true,
                        Ok(_) => return Poll::NotReady,
                        Err(e) => return Poll::Err(e),
                    }
                }
                match self.reader.read(task, &mut self.buf) {
                    Ok(0) => self.read_done = true,
                    Ok(i) => {
                        self.pos = 0;
                        self.cap = i;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        self.read_ready = false;
                        return Poll::NotReady
                    }
                    Err(e) => return Poll::Err(e),
                }
            }

            // Now that our buffer has some data, let's write it out!
            while self.pos < self.cap || (self.read_done && !self.flush_done) {
                if !self.write_ready {
                    match try_poll!(self.writer.poll(task)) {
                        Ok(Some(ref r)) if r.is_write() => self.write_ready = true,
                        Ok(_) => return Poll::NotReady,
                        Err(e) => return Poll::Err(e),
                    }
                }
                if self.pos == self.cap {
                    match self.writer.flush(task) {
                        Ok(()) => self.flush_done = true,
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            return Poll::NotReady
                        }
                        Err(e) => return Poll::Err(e),
                    }
                    break
                }
                match self.writer.write(task, &self.buf[self.pos..self.cap]) {
                    Ok(i) => {
                        self.pos += i;
                        self.amt += i as u64;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        self.write_ready = false;
                        return Poll::NotReady
                    }
                    Err(e) => return Poll::Err(e),
                }
            }

            if self.read_done && self.flush_done {
                return Poll::Ok(self.amt)
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if self.read_ready && self.write_ready {
            task.notify();
        }
        if !self.read_ready && !self.read_done {
            self.reader.schedule(task);
        }
        if !self.write_ready && !self.flush_done {
            self.writer.schedule(task);
        }
    }
}
