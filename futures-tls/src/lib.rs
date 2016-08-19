//! Async TLS streams
//!
//! This library is an implementation of TLS streams using the most appropriate
//! system library by default for negotiating the connection. That is, on
//! Windows this library uses SChannel, on OSX it uses SecureTransport, and on
//! other platforms it uses OpenSSL. The usage of OpenSSL can optionally be
//! forced with the `force-openssl` feature of this crate.
//!
//! Each TLS stream implements traits from the `futures_io` crate to interact
//! and interoperate with the rest of the futures I/O ecosystem. Client
//! connections initiated from this crate verify hostnames automatically and by
//! default.

#![deny(missing_docs)]

extern crate futures;
extern crate futures_io;
#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate log;

use std::io::{self, Read, Write};

use futures::{Poll, Future};

cfg_if! {
    if #[cfg(any(feature = "force-openssl",
                 all(not(target_os = "macos"),
                     not(target_os = "windows"))))] {
        mod openssl;
        use self::openssl as imp;

        /// Backend-specific extension traits.
        pub mod backend {

            /// Extension traits specific to the OpenSSL backend.
            pub mod openssl {
                pub use openssl::ServerContextExt;
                pub use openssl::ClientContextExt;
            }
        }
    } else if #[cfg(target_os = "macos")] {
        mod secure_transport;
        use self::secure_transport as imp;

        /// Backend-specific extension traits.
        pub mod backend {

            /// Extension traits specific to the SecureTransport backend.
            pub mod secure_transport {
                pub use secure_transport::ServerContextExt;
                pub use secure_transport::ClientContextExt;
            }
        }
    } else {
        mod schannel;
        use self::schannel as imp;

        /// Backend-specific extension traits.
        pub mod backend {

            /// Extension traits specific to the SChannel backend.
            pub mod schannel {
                pub use schannel::ServerContextExt;
                pub use schannel::ClientContextExt;
            }
        }
    }
}

/// A context used to initiate server-side connections of a TLS server.
///
/// Server contexts are typically much harder to create than a client context
/// because they need to know the public/private key that they're going to
/// negotiate the connection with. Specifying these keys is typically done in a
/// very backend-specific method, unfortunately. For that reason there's no
/// `new` constructor.
///
/// For some examples of how to create a context, though, you can take a look at
/// the test suite of `futures-tls`.
pub struct ServerContext {
    inner: imp::ServerContext,
}

/// A context used to initiate client-side connections to a TLS server.
///
/// Client context by default perform hostname validation of connections issued
/// by ensuring that certificates sent by the server are trusted by the system
/// and are indeed valid.
///
/// Normally a `ClientContext` doesn't need extra configuration, but extra
/// configuration can be performed by the backend-specific extension traits.
pub struct ClientContext {
    inner: imp::ClientContext,
}

/// A wrapper around an underlying raw stream which implements the TLS or SSL
/// protocol.
///
/// A `TlsStream<S>` represents a handshake that has been completed successfully
/// and both the server and the client are ready for receiving and sending
/// data. Bytes read from a `TlsStream` are decrypted from `S` and bytes written
/// to a `TlsStream` are encrypted when passing through to `S`.
pub struct TlsStream<S> {
    inner: imp::TlsStream<S>,
}

/// A future returned from `ClientContext::handshake` used to represent an
/// in-progress TLS handshake.
///
/// This future will resolve to a `TlsStream<S>` once the handshake is completed
/// or an I/O error if it otherwise cannot be completed (or verified).
pub struct ClientHandshake<S> {
    inner: imp::ClientHandshake<S>,
}

/// A future returned from `ServerContext::handshake` used to represent an
/// in-progress TLS handshake.
///
/// This future will resolve to a `TlsStream<S>` once the handshake is completed
/// or an I/O error if it otherwise cannot be completed (or verified).
pub struct ServerHandshake<S> {
    inner: imp::ServerHandshake<S>,
}

impl ClientContext {
    /// Creates a new client context ready for connecting to a remote server.
    ///
    /// The client context can be optionally configured through backend-specific
    /// extension traits, but by default client contexts will verify
    /// certificates against the system certificate store and otherwise verify
    /// that certificates are indeed valid.
    pub fn new() -> io::Result<ClientContext> {
        imp::ClientContext::new().map(|s| ClientContext { inner: s })
    }

    /// Performs a handshake with the given I/O stream to resolve to an actual
    /// I/O stream.
    ///
    /// This function will consume this context and return a future which will
    /// either resolve to a `TlsStream<S>` ready for reading/writing if the
    /// handshake completes successfully, or an error if an erroneous event
    /// otherwise happens.
    ///
    /// The provided `domain` argument is the domain that this client intended
    /// to connect to. The TLS/SSL backend will verify that the certificate
    /// provided by the server indeed matches the `domain` provided. The
    /// returned future will only resolve successfully if the domain matches,
    /// otherwise an error will be returned.
    ///
    /// The given I/O stream should be a freshly connected client (typically a
    /// TCP stream) ready to negotiate the TLS connection.
    pub fn handshake<S>(self,
                        domain: &str,
                        stream: S)
                        -> ClientHandshake<S>
        where S: Read + Write,
    {
        ClientHandshake { inner: self.inner.handshake(domain, stream) }
    }
}

impl ServerContext {
    /// Performs a handshake with the given I/O stream to resolve to an actual
    /// I/O stream.
    ///
    /// This function will consume this context and return a future which will
    /// either resolve to a `TlsStream<S>` ready for reading/writing if the
    /// handshake completes successfully, or an error if an erroneous event
    /// otherwise happens.
    ///
    /// The given I/O stream should be an accepted client of this server which
    /// is ready to negotiate the TLS connection.
    pub fn handshake<S>(self, stream: S) -> ServerHandshake<S>
        where S: Read + Write,
    {
        ServerHandshake { inner: self.inner.handshake(stream) }
    }
}

impl<S> Future for ClientHandshake<S>
    where S: Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<TlsStream<S>, io::Error> {
        self.inner.poll().map(|s| TlsStream { inner: s })
    }
}

impl<S> Future for ServerHandshake<S>
    where S: Read + Write,
{
    type Item = TlsStream<S>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<TlsStream<S>, io::Error> {
        self.inner.poll().map(|s| TlsStream { inner: s })
    }
}

impl<S> Read for TlsStream<S>
    where S: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<S> Write for TlsStream<S>
    where S: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
