extern crate openssl;
extern crate openssl_verify;
extern crate futures;

use std::io::{self, Read, Write, Error, ErrorKind};
use std::mem;

use self::openssl::crypto::pkey::PKey;
use self::openssl::ssl::{SSL_OP_NO_SSLV2, SSL_OP_NO_SSLV3, SSL_OP_NO_COMPRESSION};
use self::openssl::ssl::{self, IntoSsl, SSL_VERIFY_PEER};
use self::openssl::x509::X509;
use self::openssl_verify::verify_callback;
use futures::{Poll, Future};

pub struct ServerContext {
    inner: ssl::SslContext,
}

pub struct ClientContext {
    inner: ssl::SslContext,
}

fn cx_new() -> io::Result<ssl::SslContext> {
    let mut cx = try!(ssl::SslContext::new(ssl::SslMethod::Sslv23)
                          .map_err(translate_ssl));

    // lifted from rust-native-tls
    cx.set_options(SSL_OP_NO_SSLV2 |
                   SSL_OP_NO_SSLV3 |
                   SSL_OP_NO_COMPRESSION);
    let list = "ALL!EXPORT!EXPORT40!EXPORT56!aNULL!LOW!RC4@STRENGTH";
    try!(cx.set_cipher_list(list).map_err(translate_ssl));

    Ok(cx)
}

impl ServerContext {
    pub fn handshake<S>(self, stream: S) -> ServerHandshake<S>
        where S: Read + Write,
    {
        let res = ssl::SslStream::accept(&self.inner, stream);
        debug!("server handshake");
        ServerHandshake {
            inner: Handshake::new(res),
        }
    }
}

fn stack2handshake<S>(err: openssl::error::ErrorStack) -> ssl::HandshakeError<S> {
    ssl::HandshakeError::Failure(ssl::error::Error::Ssl(err))
}

impl ClientContext {
    pub fn new() -> io::Result<ClientContext> {
        let mut cx = try!(cx_new());
        try!(cx.set_default_verify_paths().map_err(translate_ssl));
        Ok(ClientContext { inner: cx })
    }

    pub fn handshake<S>(self,
                        domain: &str,
                        stream: S) -> ClientHandshake<S>
        where S: Read + Write,
    {
        // see rust-native-tls for the specifics here
        debug!("client handshake with {:?}", domain);
        let res = self.inner.into_ssl()
                      .map_err(stack2handshake)
                      .and_then(|mut ssl| {
            try!(ssl.set_hostname(domain).map_err(stack2handshake));
            let domain = domain.to_owned();
            ssl.set_verify_callback(SSL_VERIFY_PEER, move |p, x| {
                verify_callback(&domain, p, x)
            });
            ssl::SslStream::connect(ssl, stream)
        });
        ClientHandshake { inner: Handshake::new(res) }
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
    Stream(ssl::SslStream<S>),
    Interrupted(ssl::MidHandshakeSslStream<S>),
    Empty,
}

impl<S> Handshake<S> {
    fn new(res: Result<ssl::SslStream<S>, ssl::HandshakeError<S>>)
           -> Handshake<S> {
        match res {
            Ok(s) => Handshake::Stream(s),
            Err(ssl::HandshakeError::Failure(e)) => {
                Handshake::Error(Error::new(ErrorKind::Other, e))
            }
            Err(ssl::HandshakeError::Interrupted(s)) => {
                Handshake::Interrupted(s)
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

impl<S> Future for Handshake<S>
    where S: Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<TlsStream<S>, io::Error> {
        debug!("let's see how the handshake went");
        let stream = match mem::replace(self, Handshake::Empty) {
            Handshake::Error(e) => return Poll::Err(e),
            Handshake::Empty => panic!("can't poll handshake twice"),
            Handshake::Stream(s) => return Poll::Ok(TlsStream::new(s)),
            Handshake::Interrupted(s) => s,
        };

        // TODO: dedup with Handshake::new
        debug!("openssl handshake again");
        match stream.handshake() {
            Ok(s) => Poll::Ok(TlsStream::new(s)),
            Err(ssl::HandshakeError::Failure(e)) => {
                debug!("openssl handshake failure: {:?}", e);
                Poll::Err(Error::new(ErrorKind::Other, e))
            }
            Err(ssl::HandshakeError::Interrupted(s)) => {
                debug!("handshake not completed");
                *self = Handshake::Interrupted(s);
                Poll::NotReady
            }
        }
    }
}

fn translate_ssl(err: openssl::error::ErrorStack) -> Error {
    Error::new(io::ErrorKind::Other, err)
}

fn translate(err: openssl::ssl::Error) -> Error {
    match err {
        openssl::ssl::Error::WantRead(i) |
        openssl::ssl::Error::WantWrite(i) => return i,
        _ => Error::new(io::ErrorKind::Other, err),
    }
}

pub struct TlsStream<S> {
    inner: ssl::SslStream<S>,
}

impl<S> TlsStream<S> {
    fn new(s: ssl::SslStream<S>) -> TlsStream<S> {
        TlsStream {
            inner: s,
        }
    }
}

impl<S: Read + Write> Read for TlsStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.ssl_read(buf).map_err(translate)
    }
}

impl<S: Read + Write> Write for TlsStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.ssl_write(buf).map_err(translate)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Extension trait for servers backed by OpenSSL.
pub trait ServerContextExt: Sized {
    /// Creates a new server context given the public/private key pair.
    ///
    /// This will create a new server connection which will send `cert` to
    /// clients and use `key` as the corresponding private key to encrypt and
    /// sign communications.
    fn new(cert: &X509, key: &PKey) -> io::Result<Self>;

    /// Gets a mutable reference to the underlying SSL context, allowing further
    /// configuration.
    ///
    /// The SSL context here will eventually get used to initiate the server
    /// connection.
    fn ssl_context_mut(&mut self) -> &mut ssl::SslContext;
}

impl ServerContextExt for ::ServerContext {
    fn new(cert: &X509, key: &PKey) -> io::Result<::ServerContext> {
        let mut cx = try!(cx_new());
        try!(cx.set_certificate(cert).map_err(translate_ssl));
        try!(cx.set_private_key(key).map_err(translate_ssl));
        try!(cx.check_private_key().map_err(translate_ssl));
        Ok(::ServerContext { inner: ServerContext { inner: cx } })
    }

    fn ssl_context_mut(&mut self) -> &mut ssl::SslContext {
        &mut self.inner.inner
    }
}

/// Extension trait for clients backed by OpenSSL.
pub trait ClientContextExt {
    /// Gets a mutable reference to the underlying SSL context, allowing further
    /// configuration.
    ///
    /// The SSL context here will eventually get used to initiate the client
    /// connection, and it will otherwise be configured to validate the hostname
    /// given to `handshake` by default.
    fn ssl_context_mut(&mut self) -> &mut ssl::SslContext;
}

impl ClientContextExt for ::ClientContext {
    fn ssl_context_mut(&mut self) -> &mut ssl::SslContext {
        &mut self.inner.inner
    }
}

/// Extension trait for streams backed by OpenSSL.
pub trait TlsStreamExt {
    /// Gets a shared reference to the underlying SSL context, allowing further
    /// configuration and/or inspection of the SSL/TLS state.
    fn ssl_context(&self) -> &ssl::Ssl;
}

impl<S> TlsStreamExt for ::TlsStream<S> {
    fn ssl_context(&self) -> &ssl::Ssl {
        self.inner.inner.ssl()
    }
}
