use std::io;

use futures::{Future, Poll, Task};

use {ReadTask, WriteTask};

/// A future which will copy all data from a reader into a writer.
///
/// Created by the `copy` function, this future will resolve to the number of
/// bytes copied or an error if one happens.
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
    buf: Box<[u8]>,
}

/// Creates a future which represents copying all the bytes from one object to
/// another.
///
/// The returned future will copy all the bytes read from `reader` into the
/// `writer` specified. This future will only complete once the `reader` has hit
/// EOF and all bytes have been written to and flushed from the `writer`
/// provided.
///
/// On success the number of bytes is returned and the `reader` and `writer` are
/// consumed. On error the error is returned and the I/O objects are consumed as
/// well.
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
        buf: Box::new([0; 2048]),
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
                    debug!("copy waiting for read");
                    match try_poll!(self.reader.poll(task)) {
                        Ok(Some(ref r)) if r.is_read() => self.read_ready = true,
                        Ok(_) => return Poll::NotReady,
                        Err(e) => return Poll::Err(e),
                    }
                }
                match self.reader.read(task, &mut self.buf) {
                    Ok(0) => {
                        debug!("copy at eof");
                        self.read_done = true;
                    }
                    Ok(i) => {
                        debug!("read {} bytes", i);
                        self.pos = 0;
                        self.cap = i;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        debug!("read is gone");
                        self.read_ready = false;
                        return Poll::NotReady
                    }
                    Err(e) => return Poll::Err(e),
                }
            }

            // Now that our buffer has some data, let's write it out!
            while self.pos < self.cap || (self.read_done && !self.flush_done) {
                if !self.write_ready {
                    debug!("copy waiting for write");
                    match try_poll!(self.writer.poll(task)) {
                        Ok(Some(ref r)) if r.is_write() => self.write_ready = true,
                        Ok(_) => return Poll::NotReady,
                        Err(e) => return Poll::Err(e),
                    }
                }
                if self.pos == self.cap {
                    match self.writer.flush(task) {
                        Ok(()) => {
                            debug!("flush done");
                            self.flush_done = true;
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            debug!("waiting for another flush");
                            return Poll::NotReady
                        }
                        Err(e) => return Poll::Err(e),
                    }
                    break
                }
                match self.writer.write(task, &self.buf[self.pos..self.cap]) {
                    Ok(i) => {
                        debug!("wrote {} bytes", i);
                        self.pos += i;
                        self.amt += i as u64;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        debug!("write no longer ready");
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
