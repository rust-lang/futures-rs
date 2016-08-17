use std::io::{self, Read, Write};
use std::sync::Arc;

use futures::{Future, Poll};
use futures::stream::{Stream, Fuse};

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
    where R: Read + 'static,
          P: Parse
{
    pub fn new(source: R) -> ParseStream<R, P> {
        ParseStream {
            source: source,
            parser: Default::default(),
            buf: Arc::new(Vec::with_capacity(2048)),
            pos: 0,
            need_parse: false,
            eof: false,
        }
    }
}

impl<R, P> Stream for ParseStream<R, P>
    where R: Read + 'static,
          P: Parse
{
    type Item = P;
    type Error = P::Error;

    fn poll(&mut self) -> Poll<Option<P>, P::Error> {
        loop {
            if self.need_parse {
                debug!("attempting to parse");
                match P::parse(&mut self.parser, &self.buf, self.pos) {
                    Some(Ok((i, n))) => {
                        debug!("parsed an item, consuming {} bytes", n);
                        self.pos += n;
                        return Poll::Ok(Some(i))
                    }
                    Some(Err(e)) => return Poll::Err(e),
                    None => self.need_parse = false,
                }
                debug!("parse not ready");

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

            let buf = Arc::get_mut(&mut self.buf).unwrap();
            let before = buf.len();
            match read(&mut self.source, buf) {
                Ok(()) if buf.len() == before => self.eof = true,
                Ok(()) => self.need_parse = true,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if before == buf.len() {
                        return Poll::NotReady
                    }
                    self.need_parse = true;
                }
                Err(e) => return Poll::Err(e.into()),
            }
        }
    }
}

// TODO: move this into method
fn read(socket: &mut Read, input: &mut Vec<u8>) -> io::Result<()> {
    match try!(socket.read(unsafe { slice_to_end(input) })) {
        0 => return Ok(()),
        n => {
            trace!("socket read {} bytes", n);
            unsafe {
                let len = input.len();
                input.set_len(len + n);
            }
            return Ok(())
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
    items: Fuse<S>,
    buf: Vec<u8>,
}

impl<W, S> StreamWriter<W, S>
    where W: Write + 'static,
          S: Stream,
          S::Item: Serialize,
          S::Error: From<io::Error>
{
    pub fn new(sink: W, items: S) -> StreamWriter<W, S> {
        StreamWriter {
            sink: sink,
            items: items.fuse(),
            buf: Vec::with_capacity(2048),
        }
    }
}

impl<W, S> Future for StreamWriter<W, S>
    where W: Write + 'static,
          S: Stream,
          S::Item: Serialize,
          S::Error: From<io::Error>
{
    type Item = ();
    type Error = S::Error;

    fn poll(&mut self) -> Poll<(), S::Error> {
        // First up, grab any responses we have and serialize them into our
        // local buffer. Make sure to pass down `tokens` only on the *first*
        // poll for items
        //
        // TODO: limit this loop on the size of `self.buf`.
        loop {
            match self.items.poll() {
                Poll::Err(e) => return Poll::Err(e),
                Poll::Ok(Some(item)) => {
                    debug!("got an item to serialize!");
                    item.serialize(&mut self.buf);
                }
                Poll::Ok(None) |
                Poll::NotReady => break,
            }
        }

        // Now that we might have some responses to write, try to write them.
        // Note that we always execute at least one iteration of this loop to
        // attempt to pull out write readiness ASAP. If a response is being
        // calculated we can learn that we're ready for a write immediately and
        // write as soon as it's ready when it comes around.
        //
        // If, however, we have no data to write and there are no more responses
        // that will come out, then we don't need to wait for write readiness.
        loop {
            debug!("wut: {}", self.buf.len());
            if self.buf.len() == 0 {
                if self.items.is_done() {
                    return Poll::Ok(())
                } else {
                    return Poll::NotReady
                }
            }

            debug!("trying to write some data");
            if let Err(e) = write(&mut self.sink, &mut self.buf) {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return Poll::NotReady
                } else {
                    return Poll::Err(e.into())
                }
            }
        }
    }
}

// TODO: make this a method
fn write(sink: &mut Write, buf: &mut Vec<u8>) -> io::Result<()> {
    loop {
        match try!(sink.write(&buf)) {
            0 => {
                // TODO: copied from mio example, clean up
                return Err(io::Error::new(io::ErrorKind::Other, "early eof2"));
            }
            n => {
                // TODO: consider draining more lazily, i.e. only just before
                //       returning
                buf.drain(..n);
                if buf.len() == 0 {
                    return Ok(());
                }
            }
        }
    }
}
