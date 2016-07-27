use std::io;
use std::mem;

use {Poll, Task, Future};
use io::ReadTask;

pub struct ReadToEnd<A> {
    a: A,
    buf: Vec<u8>,
    first: bool,
}

pub fn read_to_end<A>(a: A, buf: Vec<u8>) -> ReadToEnd<A>
    where A: ReadTask,
{
    ReadToEnd {
        a: a,
        buf: buf,
        first: true,
    }
}

impl<A> Future for ReadToEnd<A>
    where A: ReadTask,
{
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Vec<u8>, io::Error> {
        // Right now I/O streams are all "edge ready" in the sense that once
        // they emit a readable notification, we may never get another readable
        // notification until all the data there was read out.
        //
        // We don't know what the state of the I/O object passed to this
        // combinator is, so the first time we `poll` we *must* perform a
        // `read`. After we've learned that there's nothing else to read though
        // we can wait for readable notifications from the underlying stream.
        if self.first {
            self.first = false;
        } else {
            match try_poll!(self.a.poll(task)) {
                Ok(Some(ref r)) if r.is_read() => {}
                Ok(Some(r)) => {
                    debug!("notification but not readable {:?}", r);
                    return Poll::NotReady
                }
                Ok(None) => {
                    return Poll::Ok(mem::replace(&mut self.buf, Vec::new()))
                }
                Err(e) => return Poll::Err(e)
            }
        }

        debug!("attempting a read to end");
        let start = self.buf.len();
        match self.a.read_to_end(task, &mut self.buf) {
            // If we get `Ok`, then we know the stream hit EOF, so we're done
            Ok(_) => Poll::Ok(mem::replace(&mut self.buf, Vec::new())),

            // If we hit WouldBlock, then the data we read so far is in the
            // buffer and we just need to wait until we're readable again.
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                debug!("read {} bytes, waiting for more",
                       self.buf.len() - start);
                Poll::NotReady
            }

            Err(e) => Poll::Err(e)
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        self.a.schedule(task)
    }
}
