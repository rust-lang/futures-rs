extern crate openssl;
extern crate futures;

use std::io::{self, Read, Write, Error, ErrorKind};

use openssl::ssl;
use futures::{Poll, Task};
use futures::stream::Stream;
use futures::io::Ready;

pub struct SslStream<S> {
    inner: ssl::SslStream<S>,
}

impl<S> SslStream<S>
    where S: Read + Write + Stream<Item=Ready, Error=Error>,
{
    pub fn connect<T: ssl::IntoSsl>(ssl: T, stream: S) -> io::Result<SslStream<S>> {
        ssl::SslStream::connect(ssl, stream)
            .map(|s| SslStream { inner: s })
            .map_err(translate_ssl)
    }

    pub fn accept<T: ssl::IntoSsl>(ssl: T, stream: S) -> io::Result<SslStream<S>> {
        ssl::SslStream::accept(ssl, stream)
            .map(|s| SslStream { inner: s })
            .map_err(translate_ssl)
    }

    pub fn ssl(&self) -> &ssl::Ssl {
        self.inner.ssl()
    }
}

fn translate(err: ssl::Error) -> Error {
    let kind = match err {
        ssl::Error::WantRead(_) => ErrorKind::WouldBlock,
        ssl::Error::WantWrite(_) => ErrorKind::WouldBlock,
        _ => ErrorKind::Other,
    };
    Error::new(kind, err)
}

fn translate_ssl(err: ssl::error::SslError) -> Error {
    Error::new(io::ErrorKind::Other, err)
}

impl<S> Stream for SslStream<S>
    where S: Stream<Item=Ready, Error=Error>,
{
    type Item = Ready;
    type Error = Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        // TODO: be smarter, we know if we need to read or need to write, only
        //       return once they're satisfied.
        self.inner.get_mut().poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.inner.get_mut().schedule(task)
    }
}

impl<S: Read + Write> Read for SslStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.ssl_read(buf).map_err(translate)
    }
}

impl<S: Read + Write> Write for SslStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.ssl_write(buf).map_err(translate)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
