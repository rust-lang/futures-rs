extern crate security_framework;

use std::mem;
use std::io::{self, Read, Write, Error, ErrorKind};

use self::security_framework::base::Error as StError;
use self::security_framework::certificate::SecCertificate;
use self::security_framework::identity::SecIdentity;
use self::security_framework::secure_transport as st;
use self::security_framework::trust::TrustResult;
use futures::{Poll, Task, Future};
use futures::stream::Stream;
use futures_io::{Ready, ReadyTracker};

pub struct TlsStream<S> {
    stream: st::SslStream<ReadyTracker<S>>,
    read_wont: Wont,
    write_wont: Wont,
}

enum Wont {
    Read,
    Write,
    Both,
    Unknown,
    Notified,
}

pub struct ServerContext {
    inner: st::SslContext,
}

pub struct ClientContext {
    inner: st::SslContext,
    certs: Vec<SecCertificate>,
}

fn cx_new(side: st::ProtocolSide, kind: st::ConnectionType)
          -> io::Result<st::SslContext> {
    st::SslContext::new(side, kind).map_err(translate)
}

impl ClientContext {
    pub fn new() -> io::Result<ClientContext> {
        let cx = try!(cx_new(st::ProtocolSide::Client,
                             st::ConnectionType::Stream));
        Ok(ClientContext {
            inner: cx,
            certs: Vec::new(),
        })
    }

    pub fn handshake<S>(self,
                        domain: &str,
                        stream: S) -> ClientHandshake<S>
        where S: Read + Write + Stream<Item=Ready, Error=io::Error>,
    {
        let stream = ReadyTracker::new(stream);
        let mut inner = self.inner;
        let res = inner.set_peer_domain_name(domain)
                       .map_err(st::HandshakeError::Failure)
                       .and_then(|()| {
            inner.handshake(stream)
        });
        ClientHandshake {
            inner: Handshake::new(res, self.certs),
        }
    }
}

impl ServerContext {
    pub fn handshake<S>(self, stream: S) -> ServerHandshake<S>
        where S: Read + Write + Stream<Item=Ready, Error=io::Error>,
    {
        let stream = ReadyTracker::new(stream);
        ServerHandshake {
            inner: Handshake::new(self.inner.handshake(stream), Vec::new()),
        }
    }
}

pub struct ClientHandshake<S> {
    inner: Handshake<S>,
}

pub struct ServerHandshake<S> {
    inner: Handshake<S>,
}

enum Handshake<S> {
    Error(io::Error),
    Stream(st::SslStream<ReadyTracker<S>>),
    Interrupted(st::MidHandshakeSslStream<ReadyTracker<S>>,
                Vec<SecCertificate>),
    Empty,
}

impl<S> Handshake<S> {
    fn new(res: Result<st::SslStream<ReadyTracker<S>>,
                       st::HandshakeError<ReadyTracker<S>>>,
           certs: Vec<SecCertificate>)
           -> Handshake<S> {
        match res {
            Ok(s) => {
                assert!(certs.len() == 0);
                Handshake::Stream(s)
            }
            Err(st::HandshakeError::Failure(e)) => {
                Handshake::Error(Error::new(ErrorKind::Other, e))
            }
            Err(st::HandshakeError::Interrupted(s)) => {
                Handshake::Interrupted(s, certs)
            }
        }
    }
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
    fn validate_certs(&self,
                      stream: &st::MidHandshakeSslStream<ReadyTracker<S>>,
                      certs: &[SecCertificate]) -> io::Result<()> {
        // Copied from ClientBuilder in secure_transport.rs
        //
        // TODO: this should happen when we get an interrupted error during
        //       read/write.
        if certs.len() == 0 || !stream.server_auth_completed() {
            return Ok(())
        }

        debug!("handshake auth completed, checking for validity");
        let mut trust = try!(stream.context().peer_trust().map_err(translate));
        try!(trust.set_anchor_certificates(&certs).map_err(translate));
        let trusted = try!(trust.evaluate().map_err(translate));
        match trusted {
            TrustResult::Proceed |
            TrustResult::Unspecified => Ok(()),
            TrustResult::Invalid |
            TrustResult::OtherError => {
                Err(Error::new(ErrorKind::Other, "bad ssl request"))
            }
            TrustResult::Deny => {
                Err(Error::new(ErrorKind::Other, "trust setting denied cert"))
            }
            TrustResult::RecoverableTrustFailure |
            TrustResult::FatalTrustFailure => {
                Err(Error::new(ErrorKind::Other, "not a trusted certificate"))
            }
        }
    }
}

impl<S> Future for Handshake<S>
    where S: Stream<Item=Ready, Error=io::Error> + Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<TlsStream<S>, io::Error> {
        let (mut stream, certs) = match mem::replace(self, Handshake::Empty) {
            Handshake::Error(e) => return Poll::Err(e),
            Handshake::Empty => panic!("can't poll handshake twice"),
            Handshake::Stream(s) => return Poll::Ok(TlsStream::new(s)),
            Handshake::Interrupted(s, certs) => (s, certs),
        };

        if let Err(e) = self.validate_certs(&stream, &certs) {
            return Poll::Err(e)
        }

        match stream.get_mut().poll(task) {
            Poll::Ok(None) => panic!(), // TODO: track this
            Poll::Err(e) => return Poll::Err(e),

            // We track readiness internally in ReadyTracker, so don't pull it
            // out, and otherwise just assume we may otherwise be able to make
            // progress (e.g. we ran the validate certs callback but I/O
            // otherwise doesn't need to be ready
            Poll::Ok(Some(_r)) => {}
            Poll::NotReady => {}
        }

        // TODO: dedup with Handshake::new
        match stream.handshake() {
            Ok(s) => Poll::Ok(TlsStream::new(s)),
            Err(st::HandshakeError::Failure(e)) => {
                Poll::Err(Error::new(ErrorKind::Other, e))
            }
            Err(st::HandshakeError::Interrupted(s)) => {
                *self = Handshake::Interrupted(s, certs);
                Poll::NotReady
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        match *self {
            Handshake::Error(_) => task.notify(),
            Handshake::Empty => task.notify(),
            Handshake::Stream(_) => task.notify(),
            // TODO: does this need to take certs and server_auth_completed into
            //       account?
            Handshake::Interrupted(ref mut s, _) => s.get_mut().schedule(task),
        }
    }
}


fn translate(err: StError) -> Error {
    Error::new(ErrorKind::Other, err)
}

impl<S> TlsStream<S> {
    fn new(stream: st::SslStream<ReadyTracker<S>>) -> TlsStream<S> {
        TlsStream {
            stream: stream,
            read_wont: Wont::Unknown,
            write_wont: Wont::Unknown,
        }
    }
}

impl Wont {
    /// Right now SecureTransport tells us when it would block (via a
    /// WouldBlock error), but it unfortunately doesn't tell us *why*.
    ///
    /// When reading/writing an SSL stream we may be blocked in a write of some
    /// application data because we couldn't read data off the stream, so
    /// there's a bit of a translation layer that needs to happen as part of
    /// `poll`. This function is intended to modified the desired readiness for
    /// each half of this connection.
    ///
    /// To work around this part of SecureTransport we just record the state
    /// before and after an I/O operation, basically tracking whether WouldBlock
    /// was hit or not. Depending on which side of the stream hit WouldBlock we
    /// know what state we're going into if a WouldBlock error is actually
    /// returned.
    // TODO: this is probably somewhat lossy, for example if we hit EAGAIN but
    //       SecureTransport doesn't return that to us we may lose it and poll
    //       too much? Probably needs tests either way in any case.
    fn track<F, S, T>(&mut self,
                      arg: &mut st::SslStream<ReadyTracker<S>>,
                      f: F) -> io::Result<T>
        where F: FnOnce(&mut st::SslStream<ReadyTracker<S>>) -> io::Result<T>,
    {
        // First, read the state before we run the operation. If `get_mut`
        // returns `None` then it means the handshake has failed and we just
        // keep going.
        let read_before = arg.get_ref().maybe_read_ready();
        let write_before = arg.get_ref().maybe_write_ready();

        // Perform the operation, but only extract WouldBlock errors
        let e = match f(arg) {
            Ok(e) => return Ok(e),
            Err(e) => e,
        };
        if e.kind() != ErrorKind::WouldBlock {
            return Err(e)
        }

        // Look at how the state change, and alter ourselves accordingly.
        let read_change = read_before != arg.get_ref().maybe_read_ready();
        let write_change = write_before != arg.get_ref().maybe_write_ready();
        if read_change && write_change {
            *self = Wont::Both;
        } else if read_change {
            *self = Wont::Read;
        } else if write_change {
            *self = Wont::Write;
        } else {
            *self = Wont::Unknown;
        }
        Err(e)
    }

    // See openssl.rs for docs on this
    fn satisfied<S>(&mut self, s: &ReadyTracker<S>) -> bool {
        match mem::replace(self, Wont::Notified) {
            Wont::Read if s.maybe_read_ready() => true,
            Wont::Write if s.maybe_write_ready() => true,
            Wont::Both if s.maybe_write_ready() || s.maybe_read_ready() => true,
            Wont::Unknown => true,
            other => {
                *self = other;
                false
            }
        }
    }

    fn notified(&self) -> bool {
        match *self {
            Wont::Notified => true,
            _ => false,
        }
    }
}

// See openssl for docs on this impl
impl<S> Stream for TlsStream<S>
    where S: Stream<Item=Ready, Error=Error>,
{
    type Item = Ready;
    type Error = Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        let inner = self.stream.get_mut();
        match inner.poll(task) {
            Poll::Err(e) => return Poll::Err(e),
            Poll::Ok(None) => panic!(),
            Poll::Ok(Some(_)) => {}
            Poll::NotReady => {}
        }

        if self.read_wont.satisfied(inner) {
            if self.write_wont.satisfied(inner) {
                Poll::Ok(Some(Ready::ReadWrite))
            } else {
                Poll::Ok(Some(Ready::Read))
            }
        } else if self.write_wont.satisfied(inner) {
            Poll::Ok(Some(Ready::Write))
        } else {
            Poll::NotReady
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if !self.read_wont.notified() || !self.write_wont.notified() {
            self.stream.get_mut().schedule(task);
        }
    }
}

impl<S: Read + Write> Read for TlsStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_wont.track(&mut self.stream, |s| s.read(buf))
    }
}

impl<S: Read + Write> Write for TlsStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_wont.track(&mut self.stream, |s| s.write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write_wont.track(&mut self.stream, |s| s.flush())
    }
}

pub trait ServerContextExt: Sized {
    fn new(identity: &SecIdentity,
           certs: &[SecCertificate]) -> io::Result<Self>;
    fn ssl_context_mut(&mut self) -> &mut st::SslContext;
}

impl ServerContextExt for ::ServerContext {
    fn new(identity: &SecIdentity,
           certs: &[SecCertificate]) -> io::Result<::ServerContext> {
        let mut cx = try!(cx_new(st::ProtocolSide::Server,
                                 st::ConnectionType::Stream));
        try!(cx.set_certificate(&identity, certs)
               .map_err(translate));
        Ok(::ServerContext { inner: ServerContext { inner: cx } })
    }

    fn ssl_context_mut(&mut self) -> &mut st::SslContext {
        &mut self.inner.inner
    }
}

pub trait ClientContextExt {
    fn anchor_certificates(&mut self,
                           certs: &[SecCertificate]) -> io::Result<()>;
    fn ssl_context_mut(&mut self) -> &mut st::SslContext;
}

impl ClientContextExt for ::ClientContext {
    fn anchor_certificates(&mut self,
                           certs: &[SecCertificate]) -> io::Result<()> {
        try!(self.inner.inner.set_break_on_server_auth(true).map_err(translate));
        self.inner.certs.extend(certs.iter().cloned());
        Ok(())
    }

    fn ssl_context_mut(&mut self) -> &mut st::SslContext {
        &mut self.inner.inner
    }
}

pub trait TlsStreamExt {
    fn ssl_context(&self) -> &st::SslContext;
    fn ssl_context_mut(&mut self) -> &mut st::SslContext;
}

impl<S> TlsStreamExt for ::TlsStream<S> {
    fn ssl_context(&self) -> &st::SslContext {
        self.inner.stream.context()
    }

    fn ssl_context_mut(&mut self) -> &mut st::SslContext {
        self.inner.stream.context_mut()
    }
}
