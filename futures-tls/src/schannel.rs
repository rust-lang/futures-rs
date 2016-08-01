extern crate schannel;

use std::io::{self, Read, Write, Error};
use std::mem;

use self::schannel::tls_stream::{self, HandshakeError};
use self::schannel::tls_stream::MidHandshakeTlsStream;
use self::schannel::schannel_cred::{self, Direction};
use futures::{Poll, Task, Future};
use futures::stream::Stream;
use futures_io::Ready;

pub struct ServerContext {
    cred: schannel_cred::Builder,
    stream: tls_stream::Builder,
}

pub struct ClientContext {
    cred: schannel_cred::Builder,
    stream: tls_stream::Builder,
}

impl ServerContext {
    pub fn handshake<S>(mut self, stream: S) -> ServerHandshake<S>
        where S: Read + Write + Stream<Item=Ready, Error=io::Error>,
    {
        let res = self.cred.acquire(Direction::Inbound);
        let res = res.map_err(HandshakeError::Failure);
        let res = res.and_then(|cred| {
            self.stream.accept(cred, stream)
        });
        ServerHandshake { inner: Handshake::new(res) }
    }
}


impl ClientContext {
    pub fn new() -> io::Result<ClientContext> {
        Ok(ClientContext {
            cred: schannel_cred::Builder::new(),
            stream: tls_stream::Builder::new(),
        })
    }

    pub fn handshake<S>(mut self,
                        domain: &str,
                        stream: S) -> ClientHandshake<S>
        where S: Read + Write + Stream<Item=Ready, Error=io::Error>,
    {
        let res = self.cred.acquire(Direction::Outbound);
        let res = res.map_err(HandshakeError::Failure);
        let res = res.and_then(|cred| {
            self.stream.domain(domain).connect(cred, stream)
        });
        ClientHandshake { inner: Handshake::new(res) }
    }
}

pub struct ServerHandshake<S> {
    inner: Handshake<S>,
}

pub struct ClientHandshake<S> {
    inner: Handshake<S>,
}

enum Handshake<S> {
    Error(io::Error),
    Stream(tls_stream::TlsStream<S>),
    Interrupted(MidHandshakeTlsStream<S>),
    Empty,
}

impl<S> Future for ClientHandshake<S>
    where S: Stream<Item=Ready, Error=io::Error> + Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<TlsStream<S>, io::Error> {
        self.inner.poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.inner.schedule(task)
    }
}

impl<S> Future for ServerHandshake<S>
    where S: Stream<Item=Ready, Error=io::Error> + Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<TlsStream<S>, io::Error> {
        self.inner.poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.inner.schedule(task)
    }
}

impl<S> Handshake<S> {
    fn new(res: Result<tls_stream::TlsStream<S>, HandshakeError<S>>)
           -> Handshake<S> {
        match res {
            Ok(s) => Handshake::Stream(s),
            Err(HandshakeError::Failure(e)) => Handshake::Error(e),
            Err(HandshakeError::Interrupted(s)) => Handshake::Interrupted(s),
        }
    }
}

impl<S> Future for Handshake<S>
    where S: Stream<Item=Ready, Error=io::Error> + Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<TlsStream<S>, io::Error> {
        let mut stream = match mem::replace(self, Handshake::Empty) {
            Handshake::Error(e) => return Poll::Err(e),
            Handshake::Empty => panic!("can't poll handshake twice"),
            Handshake::Stream(s) => return Poll::Ok(TlsStream::new(s)),
            Handshake::Interrupted(s) => s,
        };

        match stream.get_mut().poll(task) {
            Poll::Ok(None) => panic!(), // TODO: track this
            Poll::Err(e) => return Poll::Err(e),
            Poll::Ok(Some(_r)) => {}
            Poll::NotReady => {}
        }

        // TODO: dedup with Handshake::new
        match stream.handshake() {
            Ok(s) => Poll::Ok(TlsStream::new(s)),
            Err(HandshakeError::Failure(e)) => Poll::Err(e),
            Err(HandshakeError::Interrupted(s)) => {
                *self = Handshake::Interrupted(s);
                Poll::NotReady
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        match *self {
            Handshake::Error(_) => task.notify(),
            Handshake::Empty => task.notify(),
            Handshake::Stream(_) => task.notify(),
            Handshake::Interrupted(ref mut s) => s.get_mut().schedule(task),
        }
    }
}

pub struct TlsStream<S> {
    inner: tls_stream::TlsStream<S>,
}

impl<S> TlsStream<S> {
    fn new(s: tls_stream::TlsStream<S>) -> TlsStream<S> {
        TlsStream { inner: s }
    }
}

impl<S> Stream for TlsStream<S>
    where S: Stream<Item=Ready, Error=Error> + Read + Write,
{
    type Item = Ready;
    type Error = Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        self.inner.get_mut().poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.inner.get_mut().schedule(task)
    }
}

impl<S: Read + Write> Read for TlsStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<S: Read + Write> Write for TlsStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub trait ServerContextExt: Sized {
    fn new() -> Self;
    fn schannel_cred(&mut self) -> &mut schannel_cred::Builder;
    fn tls_stream(&mut self) -> &mut tls_stream::Builder;
}

impl ServerContextExt for ::ServerContext {
    fn new() -> ::ServerContext {
        ::ServerContext {
            inner: ServerContext {
                cred: schannel_cred::Builder::new(),
                stream: tls_stream::Builder::new(),
            },
        }
    }

    fn schannel_cred(&mut self) -> &mut schannel_cred::Builder {
        &mut self.inner.cred
    }

    fn tls_stream(&mut self) -> &mut tls_stream::Builder {
        &mut self.inner.stream
    }
}

pub trait ClientContextExt {
    fn schannel_cred(&mut self) -> &mut schannel_cred::Builder;
    fn tls_stream(&mut self) -> &mut tls_stream::Builder;
}

impl ClientContextExt for ::ClientContext {
    fn schannel_cred(&mut self) -> &mut schannel_cred::Builder {
        &mut self.inner.cred
    }

    fn tls_stream(&mut self) -> &mut tls_stream::Builder {
        &mut self.inner.stream
    }
}

pub trait TlsStreamExt {
}

impl<S> TlsStreamExt for ::TlsStream<S> {
}
