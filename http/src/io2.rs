use std::io::{self, Read, Write};
use std::mem;
use std::sync::Arc;

use futures::{Future, Wake, Tokens};
use futures::stream::{Stream, StreamResult, Fuse};
use futuremio::*;

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
    source_ready: ReadinessStream,
    parser: P::Parser,
    buf: Vec<u8>,
    last_buf: Option<Arc<Vec<u8>>>,

    // how far into the buffer have we parsed? note: we drain lazily
    pos: usize,

    // is there new data that we need to try to parse?
    need_parse: bool,

    // has `source` yielded an EOF?
    eof: bool,
}

impl<R, P> ParseStream<R, P>
    where R: Read + Send + 'static,
          P: Parse
{
    pub fn new(source: R, source_ready: ReadinessStream) -> ParseStream<R, P> {
        ParseStream {
            source: source,
            source_ready: source_ready,
            parser: Default::default(),
            buf: Vec::with_capacity(2048),
            last_buf: None,
            pos: 0,
            need_parse: false,
            eof: false,
        }
    }
}

// TODO: move this into method
fn read<R: Read>(socket: &mut R, input: &mut Vec<u8>) -> io::Result<(usize, bool)> {
    loop {
        match socket.read(unsafe { slice_to_end(input) }) {
            Ok(0) => {
                trace!("socket EOF");
                return Ok((0, true))
            }
            Ok(n) => {
                trace!("socket read {} bytes", n);
                unsafe {
                    let len = input.len();
                    input.set_len(len + n);
                }
                return Ok((n, false));
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok((0, false)),
            Err(e) => return Err(e),
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
    where R: Read + Send + 'static,
          P: Parse
{
    type Item = P;
    type Error = P::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<StreamResult<P, P::Error>> {
        loop {
            if let Some(mut data) = self.last_buf.take() {
                assert!(self.buf.len() == 0);
                debug!("attempting to parse");
                match P::parse(&mut self.parser, &data, self.pos) {
                    Some(Ok((i, n))) => {
                        self.pos += n;
                        self.last_buf = Some(data);
                        return Some(Ok(Some(i)))
                    }
                    Some(Err(e)) => return Some(Err(e)),
                    None => {}
                }

                // If we didn't even get an entire request then try to keep the
                // same buffer and avoid reallocating a bunch.
                let mut swapped = false;
                if self.pos == 0 {
                    if let Some(ptr) = Arc::get_mut(&mut data) {
                        mem::swap(&mut *ptr, &mut self.buf);
                        swapped = true;
                    }
                }

                // If the offset is > 0 or the arc is referenced elsewhere
                // though then just take the data and put it on the beginning of
                // the buffer
                if !swapped {
                    self.buf.extend_from_slice(&data[self.pos..]);
                    self.pos = 0;
                }
            }

            if self.eof {
                return Some(Ok(None))
            }

            match self.source_ready.poll(tokens) {
                // TODO: consider refactoring `poll` API to make this more
                //       readable...
                None => return None,
                Some(Err(e)) => return Some(Err(e.into())),
                Some(Ok(Some(()))) => {
                    match read(&mut self.source, &mut self.buf) {
                        Ok((_n, eof)) => self.eof = eof,
                        Err(e) => return Some(Err(e.into())),
                    }
                    assert!(self.last_buf.is_none());
                    let buf = mem::replace(&mut self.buf, Vec::with_capacity(2048));
                    self.last_buf = Some(Arc::new(buf));
                }
                _ => unreachable!(),
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        // TODO: think through this carefully...
        if self.need_parse {
            // Empty tokens because in a `need_parse` situation, we'll attempt
            // to parse regardless of tokens
            wake.wake(&Tokens::empty())
        } else {
            self.source_ready.schedule(wake)
        }
    }
}

// TODO: make this a method
fn write<W: Write>(sink: &mut W, buf: &mut Vec<u8>) -> io::Result<()> {
    loop {
        match sink.write(&buf) {
            Ok(0) => {
                // TODO: copied from mio example, clean up
                return Err(io::Error::new(io::ErrorKind::Other, "early eof2"));
            }
            Ok(n) => {
                // TODO: consider draining more lazily, i.e. only just before
                //       returning
                buf.drain(..n);
                if buf.len() == 0 {
                    return Ok(());
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) => return Err(e),
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
    sink_ready: ReadinessStream,
    items: Fuse<S>,
    buf: Vec<u8>,
}

impl<W, S> StreamWriter<W, S>
    where W: Write + Send + 'static,
          S: Stream,
          S::Item: Serialize,
          S::Error: From<io::Error>
{
    pub fn new(sink: W, sink_ready: ReadinessStream, items: S) -> StreamWriter<W, S> {
        StreamWriter {
            sink: sink,
            sink_ready: sink_ready,
            items: items.fuse(),
            buf: Vec::with_capacity(2048),
        }
    }
}

impl<W, S> Future for StreamWriter<W, S>
    where W: Write + Send + 'static,
          S: Stream,
          S::Item: Serialize,
          S::Error: From<io::Error>
{
    type Item = ();
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<Result<(), S::Error>> {
        // make sure to pass down `tokens` only on the *first* poll for items
        let mut tokens_for_items = Some(tokens);
        loop {
            match self.items.poll(tokens_for_items.take().unwrap_or(&Tokens::all())) {
                Some(Err(e)) => return Some(Err(e)),
                Some(Ok(Some(item))) => {
                    debug!("got an item to serialize!");
                    item.serialize(&mut self.buf)
                }
                Some(Ok(None)) |
                None => break,
            }
        }

        // TODO: optimization for case where we just transitioned from no bytes
        // to write to having bytes to write; in that case, we should try to
        // write regardless of sink_ready.poll, because we haven't asked for a
        // readiness notifcation. Saves a trip around the event loop.

        if self.buf.len() > 0 {
            match self.sink_ready.poll(tokens) {
                Some(Err(e)) => Some(Err(e.into())),
                Some(Ok(Some(()))) => {
                    debug!("trying to write some data");
                    if let Err(e) = write(&mut self.sink, &mut self.buf) {
                        Some(Err(e.into()))
                    } else {
                        None
                    }
                }
                Some(Ok(None)) | // TODO: this should translate to an error
                None => None,
            }
        } else if self.items.is_done() {
            // Nothing more to write to sink, and no more incoming items; we're done!
            Some(Ok(()))
        } else {
            None
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        // wake up on writability only if we have something to write
        if self.buf.len() > 0 {
            self.sink_ready.schedule(wake.clone());
        }

        // for now, we are always happy to write more items into our unbounded buffer
        self.items.schedule(wake);
    }
}
