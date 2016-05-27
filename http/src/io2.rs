use std::sync::Arc;
use std::io;

use atomic::AtomicOption;
use futures::*;
use futuremio::{TcpStream, Error};

pub trait ReadFuture: Send + 'static {
    fn read(&mut self, buf: Vec<u8>)
            -> Box<Future<Item=Vec<u8>, Error=Error<Vec<u8>>>>;
}

pub trait WriteFuture: Send + 'static {
    fn write(&mut self, offset: usize, buf: Vec<u8>)
             -> Box<Future<Item=(usize, Vec<u8>),
                           Error=Error<(usize, Vec<u8>)>>>;

    fn write_all(self,
                 offset: usize,
                 buf: Vec<u8>)
                 -> Box<Future<Item=(Self, usize, Vec<u8>),
                               Error=Error<(Self, usize, Vec<u8>)>>>
        where Self: Sized
    {
        fn write_all_off<T>(mut stream: T,
                            off: usize,
                            buf: Vec<u8>)
                            -> Box<Future<Item=(T, usize, Vec<u8>),
                                          Error=Error<(T, usize, Vec<u8>)>>>
            where T: WriteFuture
        {
            let first = stream.write(off, buf);
            first.then(move |res| {
                match res {
                    Ok((off, buf)) => {
                        if off == buf.len() {
                            finished((stream, off, buf)).boxed()
                        } else {
                            write_all_off(stream, off, buf)
                        }
                    }
                    Err(err) => {
                        let (err, (off, buf)) = err.into_pair();
                        failed(Error::new(err, (stream, off, buf))).boxed()
                    }
                }
            }).boxed()
        }

        write_all_off(self, offset, buf)
    }
}

impl ReadFuture for TcpStream {
    fn read(&mut self, buf: Vec<u8>)
            -> Box<Future<Item=Vec<u8>, Error=Error<Vec<u8>>>> {
        TcpStream::read(self, buf)
    }
}

impl WriteFuture for TcpStream {
    fn write(&mut self, offset: usize, buf: Vec<u8>)
             -> Box<Future<Item=(usize, Vec<u8>),
                           Error=Error<(usize, Vec<u8>)>>> {
        TcpStream::write(self, offset, buf)
    }
}

pub struct BufWriter<T: WriteFuture> {
    inner: Arc<AtomicOption<Inner<T>>>,
}

struct Inner<T: WriteFuture> {
    inner: T,
    buf: Vec<u8>,
}

impl<T: WriteFuture> BufWriter<T> {
    pub fn new(t: T) -> BufWriter<T> {
        BufWriter {
            inner: Arc::new(AtomicOption::new(Inner {
                inner: t,
                buf: Vec::with_capacity(8 * 1024),
            })),
        }
    }

    fn inner(&self) -> io::Result<Inner<T>> {
        self.inner.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::Other,
                           "concurrent write to buffered writer")
        })
    }

    pub fn flush(&mut self) -> Box<Future<Item=(), Error=io::Error>> {
        let Inner { inner, buf } = match self.inner() {
            Ok(pair) => pair,
            Err(e) => return failed(e).boxed(),
        };
        if buf.len() > 0 {
            let dst = self.inner.clone();
            self.do_flush(inner, buf).map(move |(inner, buf)| {
                dst.put(Inner { inner: inner, buf: buf });
            }).boxed()
        } else {
            self.inner.put(Inner { inner: inner, buf: buf });
            finished(()).boxed()
        }
    }

    fn do_flush(&self, inner: T, buf: Vec<u8>)
                -> Box<Future<Item=(T, Vec<u8>), Error=io::Error>> {
        let dst = self.inner.clone();
        inner.write_all(0, buf).then(move |res| {
            match res {
                Ok((inner, _, buf)) => {
                    Ok((inner, buf))
                }
                Err(err) => {
                    let (e, (inner, amt, mut buf)) = err.into_pair();
                    buf.drain(0..amt);
                    dst.put(Inner {
                        inner: inner,
                        buf: buf,
                    });
                    Err(e)
                }
            }
        }).boxed()
    }
}

impl<T: WriteFuture> WriteFuture for BufWriter<T> {
    fn write(&mut self, offset: usize, buf: Vec<u8>)
             -> Box<Future<Item=(usize, Vec<u8>),
                           Error=Error<(usize, Vec<u8>)>>>
    {
        let Inner { inner, buf: mybuf } = match self.inner() {
            Ok(pair) => pair,
            Err(e) => return failed(Error::new(e, (offset, buf))).boxed(),
        };
        let amt = buf.len() - offset;
        let flush = if mybuf.len() + amt > mybuf.capacity() {
            self.do_flush(inner, mybuf)
        } else {
            finished((inner, mybuf)).boxed()
        };

        let dst = self.inner.clone();
        flush.then(move |res| {
            match res {
                Ok((mut inner, mut mybuf)) => {
                    let ret = if amt >= mybuf.capacity() {
                        inner.write(offset, buf)
                    } else {
                        mybuf.extend_from_slice(&buf[offset..]);
                        finished((buf.len(), buf)).boxed()
                    };
                    dst.put(Inner {
                        inner: inner,
                        buf: mybuf,
                    });
                    ret
                }
                Err(e) => failed(Error::new(e, (offset, buf))).boxed(),
            }
        }).boxed()
    }
}
