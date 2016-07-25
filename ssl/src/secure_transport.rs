extern crate security_framework;

use std::mem;
use std::io::{self, Read, Write, Error, ErrorKind};

use self::security_framework::base::Error as StError;
use self::security_framework::certificate::SecCertificate;
use self::security_framework::identity::SecIdentity;
use self::security_framework::secure_transport as st;
use self::security_framework::trust::TrustResult;
use futures::{Poll, Task};
use futures::stream::Stream;
use futures::io::{Ready, ReadyTracker};

pub struct SslStream<S> {
    state: State<S>,
    read_wont: Wont,
    write_wont: Wont,
}

enum State<S> {
    Handshake(Option<st::MidHandshakeSslStream<ReadyTracker<S>>>,
              Vec<SecCertificate>),
    Connected(st::SslStream<ReadyTracker<S>>),
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

    pub fn handshake<S>(mut self,
                        domain: &str,
                        stream: S) -> io::Result<SslStream<S>>
        where S: Read + Write + Stream<Item=Ready, Error=io::Error>,
    {
        let stream = ReadyTracker::new(stream);
        try!(self.inner.set_peer_domain_name(domain).map_err(translate));
        SslStream::new(self.inner.handshake(stream), self.certs)
    }
}
impl ServerContext {
    pub fn handshake<S>(self, stream: S) -> io::Result<SslStream<S>>
        where S: Read + Write + Stream<Item=Ready, Error=io::Error>,
    {
        let stream = ReadyTracker::new(stream);
        SslStream::new(self.inner.handshake(stream), Vec::new())
    }
}

fn translate(err: StError) -> Error {
    Error::new(ErrorKind::Other, err)
}

impl<S> SslStream<S> {
    fn new(res: Result<st::SslStream<ReadyTracker<S>>,
                       st::HandshakeError<ReadyTracker<S>>>,
           certs: Vec<SecCertificate>)
           -> io::Result<SslStream<S>> {
        let state = match res {
            Ok(stream) => State::Connected(stream),
            Err(st::HandshakeError::Failure(e)) => return Err(translate(e)),
            Err(st::HandshakeError::Interrupted(s)) => {
                State::Handshake(Some(s), certs)
            }
        };
        Ok(SslStream {
            state: state,
            read_wont: Wont::Unknown,
            write_wont: Wont::Unknown,
        })
    }
}

impl<S> State<S> {
    fn get_mut(&mut self) -> Option<&mut ReadyTracker<S>> {
        match *self {
            State::Connected(ref mut s) => Some(s.get_mut()),
            State::Handshake(Some(ref mut s), _) => Some(s.get_mut()),
            State::Handshake(None, _) => None,
        }
    }

    fn handshake(&mut self) -> io::Result<&mut st::SslStream<ReadyTracker<S>>> {
        let (handshake, certs) = match *self {
            State::Connected(ref mut s) => return Ok(s),
            State::Handshake(ref mut s, ref mut certs) => {
                let s = try!(s.take().ok_or_else(|| {
                    Error::new(ErrorKind::Other, "handshake failed")
                }));
                (s, mem::replace(certs, Vec::new()))
            }
        };

        // Copied from ClientBuilder in secure_transport.rs
        //
        // TODO: this should happen when we get an interrupted error during
        //       read/write.
        if certs.len() > 0 && handshake.server_auth_completed() {
            debug!("handshake auth completed, checking for validity");
            let mut trust = try!(handshake.context().peer_trust().map_err(translate));
            try!(trust.set_anchor_certificates(&certs).map_err(translate));
            let trusted = try!(trust.evaluate().map_err(translate));
            match trusted {
                TrustResult::Proceed |
                TrustResult::Unspecified => {}
                TrustResult::Invalid |
                TrustResult::OtherError => {
                    return Err(Error::new(ErrorKind::Other, "bad ssl request"))
                }
                TrustResult::Deny => {
                    return Err(Error::new(ErrorKind::Other,
                                          "trust setting denied cert"))
                }
                TrustResult::RecoverableTrustFailure |
                TrustResult::FatalTrustFailure => {
                    return Err(Error::new(ErrorKind::Other,
                                          "not a trusted certificate"))
                }
            }
        }

        match handshake.handshake() {
            Ok(stream) => {
                debug!("handshake is now done!");
                *self = State::Connected(stream);
                match *self {
                    State::Connected(ref mut s) => Ok(s),
                    State::Handshake(_, _) => panic!(),
                }
            }
            Err(st::HandshakeError::Failure(e)) => Err(translate(e)),
            Err(st::HandshakeError::Interrupted(s)) => {
                debug!("handshake still not done");
                *self = State::Handshake(Some(s), certs);
                Err(Error::new(ErrorKind::WouldBlock, "would block"))
            }
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
                      arg: &mut State<S>,
                      f: F) -> io::Result<T>
        where F: FnOnce(&mut State<S>) -> io::Result<T>,
    {
        // First, read the state before we run the operation. If `get_mut`
        // returns `None` then it means the handshake has failed and we just
        // keep going.
        let (read_before, write_before) = match arg.get_mut() {
            Some(s) => (s.maybe_read_ready(), s.maybe_write_ready()),
            None => (false, false),
        };

        // Perform the operation, but only extract WouldBlock errors
        let e = match f(arg) {
            Ok(e) => return Ok(e),
            Err(e) => e,
        };
        if e.kind() != ErrorKind::WouldBlock {
            return Err(e)
        }

        // Look at how the state change, and alter ourselves accordingly.
        let s = match arg.get_mut() {
            Some(s) => s,
            None => return Err(e),
        };
        let read_change = read_before != s.maybe_read_ready();
        let write_change = write_before != s.maybe_write_ready();
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
impl<S> Stream for SslStream<S>
    where S: Stream<Item=Ready, Error=Error>,
{
    type Item = Ready;
    type Error = Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        let inner = match self.state.get_mut() {
            Some(s) => s,
            None => return Poll::Ok(Some(Ready::ReadWrite)),
        };
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
            match self.state.get_mut() {
                Some(s) => s.schedule(task),
                None => task.notify(),
            }
        }
    }
}

impl<S: Read + Write> Read for SslStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_wont.track(&mut self.state, |state| {
            state.handshake().and_then(|state| state.read(buf))
        })
    }
}

impl<S: Read + Write> Write for SslStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_wont.track(&mut self.state, |state| {
            state.handshake().and_then(|state| state.write(buf))
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write_wont.track(&mut self.state, |state| {
            state.handshake().and_then(|state| state.flush())
        })
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

pub trait SslStreamExt {
    // returns None on a failed handshake
    fn ssl_context(&self) -> Option<&st::SslContext>;
    fn ssl_context_mut(&mut self) -> Option<&mut st::SslContext>;
}

impl<S> SslStreamExt for ::SslStream<S> {
    fn ssl_context(&self) -> Option<&st::SslContext> {
        match self.inner.state {
            State::Connected(ref s) => Some(s.context()),
            State::Handshake(Some(ref s), _) => Some(s.context()),
            State::Handshake(None, _) => None,
        }
    }

    fn ssl_context_mut(&mut self) -> Option<&mut st::SslContext> {
        match self.inner.state {
            State::Connected(ref mut s) => Some(s.context_mut()),
            State::Handshake(Some(ref mut s), _) => Some(s.context_mut()),
            State::Handshake(None, _) => None,
        }
    }
}
