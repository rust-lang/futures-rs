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
use futures::{Poll, Task};
use futures::stream::Stream;
use futures::io::{Ready, ReadyTracker};

pub struct ServerContext {
    inner: ssl::SslContext,
}

pub struct ClientContext {
    inner: ssl::SslContext,
}

pub struct SslStream<S> {
    inner: ssl::SslStream<ReadyTracker<S>>,
    read_wont: Wont,
    write_wont: Wont,
}

enum Wont {
    Read,
    Write,
    Unknown,
    Notified,
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
    pub fn handshake<S>(self, stream: S) -> io::Result<SslStream<S>>
        where S: Read + Write + Stream<Item=Ready, Error=io::Error>,
    {
        match ssl::SslStream::accept(&self.inner, ReadyTracker::new(stream)) {
            Ok(s) => Ok(SslStream::new(s)),
            Err(e) => Err(translate_ssl(e)),
        }
    }
}


impl ClientContext {
    pub fn new() -> io::Result<ClientContext> {
        let mut cx = try!(cx_new());
        try!(cx.set_default_verify_paths().map_err(translate_ssl));
        Ok(ClientContext { inner: cx })
    }

    pub fn handshake<S>(self,
                        domain: &str,
                        stream: S) -> io::Result<SslStream<S>>
        where S: Read + Write + Stream<Item=Ready, Error=io::Error>,
    {
        // see rust-native-tls for the specifics here
        let mut ssl = try!(self.inner.into_ssl().map_err(translate_ssl));
        try!(ssl.set_hostname(domain).map_err(translate_ssl));
        let domain = domain.to_owned();
        ssl.set_verify_callback(SSL_VERIFY_PEER, move |p, x| {
            verify_callback(&domain, p, x)
        });
        match ssl::SslStream::connect(&self.inner, ReadyTracker::new(stream)) {
            Ok(s) => Ok(SslStream::new(s)),
            Err(e) => Err(translate_ssl(e)),
        }
    }
}

fn translate_ssl(err: ssl::error::SslError) -> Error {
    Error::new(io::ErrorKind::Other, err)
}

impl<S> SslStream<S> {
    fn new(s: ssl::SslStream<ReadyTracker<S>>) -> SslStream<S> {
        SslStream {
            inner: s,
            read_wont: Wont::Unknown,
            write_wont: Wont::Unknown,
        }
    }
}

impl<S> Stream for SslStream<S>
    where S: Stream<Item=Ready, Error=Error>,
{
    type Item = Ready;
    type Error = Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        // First, fill in our underlying ReadyTracker with any readiness
        // notifications on the underlying stream.
        let inner = self.inner.get_mut();
        match inner.poll(task) {
            Poll::Err(e) => return Poll::Err(e),
            Poll::Ok(None) => panic!(), // TODO: handle this
            Poll::Ok(Some(_)) => {} // recorded in ReadyTracker
            Poll::NotReady => {}
        }

        // Now that we know whether the underlying stream is ready or not, see
        // if we can move our desires forward. See more comments in the
        // `satisfied` method below.
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
        // If we've already notified about both the read and write halves of
        // this stream, then there's no need to schedule anything. It's up to
        // the caller to drain us until we receive EAGAIN.
        //
        // Otherwise, some half needs data of *some* form to make progress, so
        // schedule the task.
        if !self.read_wont.notified() || !self.write_wont.notified() {
            self.inner.get_mut().schedule(task)
        }
    }
}

impl Wont {
    /// Translates an SSL error to an I/O error, recording along the way what
    /// I/O direction is desired, if any.
    fn map_err<T>(&mut self, res: Result<T, ssl::error::Error>)
                  -> io::Result<T> {
        res.map_err(|err| {
            let kind = match err {
                ssl::Error::WantRead(_) => {
                    *self = Wont::Read;
                    ErrorKind::WouldBlock
                }
                ssl::Error::WantWrite(_) => {
                    *self = Wont::Write;
                    ErrorKind::WouldBlock
                }
                _ => ErrorKind::Other,
            };
            Error::new(kind, err)
        })
    }

    /// Tests whether this wont is satisfied given the underlying stream
    /// readiness contained in `s`.
    ///
    /// This is used in the implementation of `poll`, and the idea behind this
    /// is that once we notify a poller that a side of this stream is ready, we
    /// never need to notify them again until we get EAGAIN essentially.
    ///
    /// The stream `s` provided will record what readiness notifications have
    /// flown past it, and turn them off once it sees an EAGAIN happen. This way
    /// when the `maybe_*_ready` methods below return true we know for a fact
    /// that at some point in the past we got a notification saying that part of
    /// the stream was ready and we haven't seen an EAGAIN for it, so it's
    /// possible that we can satisfy the wont (but not guaranteed).
    ///
    /// As a caveat, if we have an "Unknown" wont, then we know we were a just
    /// created stream, so we just assume our request is satisfied. We'll figure
    /// out later on what we actually need, if any.
    fn satisfied<S>(&mut self, s: &ReadyTracker<S>) -> bool {
        match mem::replace(self, Wont::Notified) {
            Wont::Read if s.maybe_read_ready() => true,
            Wont::Write if s.maybe_write_ready() => true,
            Wont::Unknown => true,
            other => {
                *self = other;
                false
            }
        }
    }

    /// Test whether we've already notified someone about this wont, in which
    /// case we don't need to do much more.
    fn notified(&self) -> bool {
        match *self {
            Wont::Notified => true,
            _ => false,
        }
    }
}

impl<S: Read + Write> Read for SslStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_wont.map_err(self.inner.ssl_read(buf))
    }
}

impl<S: Read + Write> Write for SslStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_wont.map_err(self.inner.ssl_write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.flush() {
            Ok(()) => Ok(()),
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    self.write_wont = Wont::Write;
                }
                Err(e)
            }
        }
    }
}

pub trait ServerContextExt: Sized {
    fn new(cert: &X509, key: &PKey) -> io::Result<Self>;
    fn ssl_context_mut(&mut self) -> &mut ssl::SslContext;
}

impl ServerContextExt for ::ServerContext {
    fn new(cert: &X509, key: &PKey) -> io::Result<::ServerContext> {
        let mut cx = try!(cx_new());
        try!(cx.set_certificate(cert).map_err(translate_ssl));
        try!(cx.set_private_key(key).map_err(translate_ssl));
        Ok(::ServerContext { inner: ServerContext { inner: cx } })
    }

    fn ssl_context_mut(&mut self) -> &mut ssl::SslContext {
        &mut self.inner.inner
    }
}

pub trait ClientContextExt {
    fn ssl_context_mut(&mut self) -> &mut ssl::SslContext;
}

impl ClientContextExt for ::ClientContext {
    fn ssl_context_mut(&mut self) -> &mut ssl::SslContext {
        &mut self.inner.inner
    }
}

pub trait SslStreamExt {
    fn ssl_context(&self) -> &ssl::Ssl;
}

impl<S> SslStreamExt for ::SslStream<S> {
    fn ssl_context(&self) -> &ssl::Ssl {
        self.inner.inner.ssl()
    }
}
