extern crate security_framework;

use std::mem;
use std::io::{self, Read, Write, Error, ErrorKind};

use self::security_framework::base::Error as StError;
use self::security_framework::certificate::SecCertificate;
use self::security_framework::identity::SecIdentity;
use self::security_framework::secure_transport as st;
use self::security_framework::trust::TrustResult;
use futures::{Poll, Future};

pub struct TlsStream<S> {
    stream: st::SslStream<S>,
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
        where S: Read + Write,
    {
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
        where S: Read + Write,
    {
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
    Stream(st::SslStream<S>),
    Interrupted(st::MidHandshakeSslStream<S>,
                Vec<SecCertificate>),
    Empty,
}

impl<S> Handshake<S> {
    fn new(res: Result<st::SslStream<S>, st::HandshakeError<S>>,
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
    where S: Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<TlsStream<S>, io::Error> {
        self.inner.poll()
    }
}

impl<S> Future for ServerHandshake<S>
    where S: Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<TlsStream<S>, io::Error> {
        self.inner.poll()
    }
}

impl<S> Handshake<S> {
    fn validate_certs(&self,
                      stream: &st::MidHandshakeSslStream<S>,
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
    where S: Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<TlsStream<S>, io::Error> {
        let (stream, certs) = match mem::replace(self, Handshake::Empty) {
            Handshake::Error(e) => return Poll::Err(e),
            Handshake::Empty => panic!("can't poll handshake twice"),
            Handshake::Stream(s) => return Poll::Ok(TlsStream::new(s)),
            Handshake::Interrupted(s, certs) => (s, certs),
        };

        if let Err(e) = self.validate_certs(&stream, &certs) {
            return Poll::Err(e)
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
}


fn translate(err: StError) -> Error {
    Error::new(ErrorKind::Other, err)
}

impl<S> TlsStream<S> {
    fn new(stream: st::SslStream<S>) -> TlsStream<S> {
        TlsStream {
            stream: stream,
        }
    }
}

impl<S: Read + Write> Read for TlsStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<S: Read + Write> Write for TlsStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

/// Extension trait for servers backed by SecureTransport.
pub trait ServerContextExt: Sized {
    /// Creates a new server given the specified `identity` and list of
    /// certificates supported by the server.
    ///
    /// The server will use this identity and certificates for the certificates
    /// to use when connections are initiated to it.
    fn new(identity: &SecIdentity,
           certs: &[SecCertificate]) -> io::Result<Self>;

    /// Gets a mutable reference to the underlying SSL context, allowing further
    /// configuration.
    ///
    /// The SSL context here will eventually get used to initiate the client
    /// connection, and it will otherwise be configured to validate the hostname
    /// given to `handshake` by default.
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

/// Extension trait for clients backed by OpenSSL.
pub trait ClientContextExt {
    /// Adds a certificate to be used when validating the trust of the remote
    /// peer.
    ///
    /// The certificates provided will be trusted when communicating with the
    /// remote host.
    fn anchor_certificates(&mut self,
                           certs: &[SecCertificate]) -> io::Result<()>;

    /// Gets a mutable reference to the underlying SSL context, allowing further
    /// configuration.
    ///
    /// The SSL context here will eventually get used to initiate the client
    /// connection, and it will otherwise be configured to validate the hostname
    /// given to `handshake` by default.
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

/// Extension trait for streams backed by SecureTransport.
pub trait TlsStreamExt {
    /// Gets a shared reference to the underlying SSL context, allowing further
    /// configuration and/or inspection of the SSL/TLS state.
    fn ssl_context(&self) -> &st::SslContext;

    /// Gets a mutable reference to the underlying SSL context, allowing further
    /// configuration and/or inspection of the SSL/TLS state.
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
