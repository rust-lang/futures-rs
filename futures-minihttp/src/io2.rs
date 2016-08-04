use std::io;
use std::sync::Arc;

use futures::{Future, Task, Poll};
use futures::stream::{Stream, Fuse};
use futures_io::{Ready, ReadTask, WriteTask};

pub trait Parse: Sized + Send + 'static {
    type Parser: Default + Send + 'static;
    type Error: Send + 'static + From<io::Error>;

    fn parse(parser: &mut Self::Parser,
             buf: &Arc<Vec<u8>>,
             offset: usize)
             -> Option<Result<(Self, usize), Self::Error>>;
}

/// A stream for parsing from an underlying reader, using an unbounded internal
/// buffer.
pub struct ParseStream<R, P: Parse> {
    source: R,
    read_ready: bool,
    parser: P::Parser,
    buf: Arc<Vec<u8>>,

    // how far into the buffer have we parsed? note: we drain lazily
    pos: usize,

    // is there new data that we need to try to parse?
    need_parse: bool,

    // has `source` yielded an EOF?
    eof: bool,
}

impl<R, P> ParseStream<R, P>
    where R: ReadTask,
          P: Parse
{
    pub fn new(source: R) -> ParseStream<R, P> {
        ParseStream {
            source: source,
            read_ready: false,
            parser: Default::default(),
            buf: Arc::new(Vec::with_capacity(2048)),
            pos: 0,
            need_parse: false,
            eof: false,
        }
    }
}

// TODO: move this into method
fn read(socket: &mut ReadTask<Item=Ready, Error=io::Error>,
        task: &mut Task,
        input: &mut Vec<u8>) -> io::Result<(usize, bool)> {
    loop {
        match try_nb!(socket.read(task, unsafe { slice_to_end(input) })) {
            Some(0) => {
                trace!("socket EOF");
                return Ok((0, true))
            }
            Some(n) => {
                trace!("socket read {} bytes", n);
                unsafe {
                    let len = input.len();
                    input.set_len(len + n);
                }
                return Ok((n, false));
            }
            None => return Ok((0, false)),
        }
    }

    unsafe fn slice_to_end(v: &mut Vec<u8>) -> &mut [u8] {
        use std::slice;
        if v.capacity() == 0 {
            v.reserve(16);
        }
        if v.capacity() == v.len() {
            v.reserve(1);
        }
        slice::from_raw_parts_mut(v.as_mut_ptr().offset(v.len() as isize),
                                  v.capacity() - v.len())
    }
}

impl<R, P> Stream for ParseStream<R, P>
    where R: ReadTask,
          P: Parse
{
    type Item = P;
    type Error = P::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<P>, P::Error> {
        loop {
            if self.need_parse {
                debug!("attempting to parse");
                match P::parse(&mut self.parser, &self.buf, self.pos) {
                    Some(Ok((i, n))) => {
                        self.pos += n;
                        return Poll::Ok(Some(i))
                    }
                    Some(Err(e)) => return Poll::Err(e),
                    None => {
                        self.need_parse = false;
                    }
                }

                // Fast path if we can get mutable access to our own current
                // buffer.
                let mut drained = false;
                if let Some(buf) = Arc::get_mut(&mut self.buf) {
                    buf.drain(..self.pos);
                    drained = true;
                }

                // If we couldn't get access above then we give ourself a new
                // buffer here.
                if !drained {
                    let mut v = Vec::with_capacity(2048);
                    v.extend_from_slice(&self.buf[self.pos..]);
                    self.buf = Arc::new(v);
                }
                self.pos = 0;
            }

            if self.eof {
                return Poll::Ok(None)
            }

            if !self.read_ready {
                match try_poll!(self.source.poll(task)) {
                    Err(e) => return Poll::Err(e.into()),
                    Ok(Some(ref r)) if r.is_read() => self.read_ready = true,
                    Ok(Some(_)) => return Poll::NotReady,
                    Ok(None) => unreachable!(),
                }
            }

            let buf = Arc::get_mut(&mut self.buf).unwrap();
            match read(&mut self.source, task, buf) {
                Ok((n, eof)) => {
                    self.eof = eof;
                    self.need_parse = self.need_parse || n > 0;
                    self.read_ready = n > 0;
                }
                Err(e) => return Poll::Err(e.into()),
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        // TODO: think through this carefully...
        if self.need_parse {
            // No tokens because in a `need_parse` situation, we'll attempt to
            // parse regardless of tokens
            task.notify();
        } else {
            self.source.schedule(task)
        }
    }
}

// TODO: make this a method
fn write(sink: &mut WriteTask<Item=Ready, Error=io::Error>,
         task: &mut Task,
         buf: &mut Vec<u8>) -> io::Result<()> {
    loop {
        match try_nb!(sink.write(task, &buf)) {
            Some(0) => {
                // TODO: copied from mio example, clean up
                return Err(io::Error::new(io::ErrorKind::Other, "early eof2"));
            }
            Some(n) => {
                // TODO: consider draining more lazily, i.e. only just before
                //       returning
                buf.drain(..n);
                if buf.len() == 0 {
                    return Ok(());
                }
            }
            None => return Ok(()),
        }
    }
}

pub trait Serialize: Send + 'static {
    fn serialize(&self, buf: &mut Vec<u8>);
}

/// Serialize a stream of items into a writer, using an unbounded internal
/// buffer.
///
/// Represented as a future which yields () on successfully writing the entire
/// stream (which requires the stream to terminate), or an error if there is any
/// error along the way.
pub struct StreamWriter<W, S> {
    sink: W,
    write_ready: bool,
    items: Fuse<S>,
    buf: Vec<u8>,
}

impl<W, S> StreamWriter<W, S>
    where W: WriteTask,
          S: Stream,
          S::Item: Serialize,
          S::Error: From<io::Error>
{
    pub fn new(sink: W, items: S) -> StreamWriter<W, S> {
        StreamWriter {
            sink: sink,
            write_ready: false,
            items: items.fuse(),
            buf: Vec::with_capacity(2048),
        }
    }
}

impl<W, S> Future for StreamWriter<W, S>
    where W: WriteTask,
          S: Stream,
          S::Item: Serialize,
          S::Error: From<io::Error>
{
    type Item = ();
    type Error = S::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<(), S::Error> {
        // First up, grab any responses we have and serialize them into our
        // local buffer. Make sure to pass down `tokens` only on the *first*
        // poll for items
        //
        // TODO: limit this loop on the size of `self.buf`.
        {
            loop {
                match self.items.poll(task) {
                    Poll::Err(e) => return Poll::Err(e),
                    Poll::Ok(Some(item)) => {
                        debug!("got an item to serialize!");
                        item.serialize(&mut self.buf);
                    }
                    Poll::Ok(None) |
                    Poll::NotReady => break,
                }
            }
        }

        // TODO: optimization for case where we just transitioned from no bytes
        // to write to having bytes to write; in that case, we should try to
        // write regardless of sink_ready.poll, because we haven't asked for a
        // readiness notifcation. Saves a trip around the event loop.

        // Now that we might have some responses to write, try to write them.
        // Note that we always execute at least one iteration of this loop to
        // attempt to pull out write readiness ASAP. If a response is being
        // calculated we can learn that we're ready for a write immediately and
        // write as soon as it's ready when it comes around.
        //
        // If, however, we have no data to write and there are no more responses
        // that will come out, then we don't need to wait for write readiness.
        loop {
            if !self.write_ready && (self.buf.len() > 0 || !self.items.is_done()) {
                match try_poll!(self.sink.poll(task)) {
                    Err(e) => return Poll::Err(e.into()),
                    Ok(Some(ref r)) if r.is_write() => self.write_ready = true,
                    Ok(Some(_)) => return Poll::NotReady,

                    // TODO: this should translate to an error
                    Ok(None) => return Poll::NotReady,
                }
            }

            // If we're here then either
            //
            // (a) write_ready is true
            // (b) the response stream is done
            //
            // If we have an empty fuffer for either of these cases then we have
            // nothing left to do, and are either done with iterating or need to
            // do some more work.
            if self.buf.len() == 0 {
                if self.items.is_done() {
                    return Poll::Ok(())
                } else {
                    return Poll::NotReady
                }
            }

            debug!("trying to write some data");
            let before = self.buf.len();
            if let Err(e) = write(&mut self.sink, task, &mut self.buf) {
                return Poll::Err(e.into())
            }

            // If we didn't fail but we also didn't write anything, then we're
            // no longer ready to write and we may need to poll some more.
            if before == self.buf.len() {
                self.write_ready = false;
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        // wake up on writability only if we have something to write
        if self.buf.len() > 0 {
            self.sink.schedule(task);
        }

        // for now, we are always happy to write more items into our unbounded buffer
        self.items.schedule(task);
    }
}
