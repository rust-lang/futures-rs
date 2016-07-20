use std::io::{self, ErrorKind, Read, Write};
use std::sync::Arc;
use std::net::{self, SocketAddr};

use futures::stream::{self, Stream};
use futures::{Future, IntoFuture, failed};
use mio;

use {IoFuture, IoStream, ReadinessPair, ReadinessStream, LoopHandle};

/// An I/O object representing a TCP socket listening for incoming connections.
///
/// This object can be converted into a stream of incoming connections for
/// various forms of processing.
pub struct TcpListener {
    loop_handle: LoopHandle,
    inner: ReadinessPair<mio::tcp::TcpListener>,
}

impl TcpListener {
    fn new(listener: mio::tcp::TcpListener,
           handle: LoopHandle) -> Box<IoFuture<TcpListener>> {
        ReadinessPair::new(handle.clone(), listener).map(|p| {
            TcpListener { loop_handle: handle, inner: p }
        }).boxed()
    }

    /// Create a new TCP listener from the standard library's TCP listener.
    ///
    /// This method can be used when the `LoopHandle::tcp_listen` method isn't
    /// sufficient because perhaps some more configuration is needed in terms of
    /// before the calls to `bind` and `listen`.
    ///
    /// This API is typically paired with the `net2` crate and the `TcpBuilder`
    /// type to build up and customize a listener before it's shipped off to the
    /// backing event loop. This allows configuration of options like
    /// `SO_REUSEPORT`, binding to multiple addresses, etc.
    ///
    /// The `addr` argument here is one of the addresses that `listener` is
    /// bound to and the listener will only be guaranteed to accept connections
    /// of the same address type currently.
    ///
    /// Finally, the `handle` argument is the event loop that this listener will
    /// be bound to.
    ///
    /// The platform specific behavior of this function looks like:
    ///
    /// * On Unix, the socket is placed into nonblocking mode and connections
    ///   can be accepted as normal
    ///
    /// * On Windows, the address is stored internally and all future accepts
    ///   will only be for the same IP version as `addr` specified. That is, if
    ///   `addr` is an IPv4 address then all sockets accepted will be IPv4 as
    ///   well (same for IPv6).
    pub fn from_listener(listener: net::TcpListener,
                         addr: &SocketAddr,
                         handle: LoopHandle) -> Box<IoFuture<TcpListener>> {
        mio::tcp::TcpListener::from_listener(listener, addr)
            .into_future()
            .and_then(|l| TcpListener::new(l, handle))
            .boxed()
    }

    /// Returns the local address that this listener is bound to.
    ///
    /// This can be useful, for example, when binding to port 0 to figure out
    /// which port was actually bound.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.source.local_addr()
    }

    /// Consumes this listener, returning a stream of the sockets this listener
    /// accepts.
    ///
    /// This method returns an implementation of the `Stream` trait which
    /// resolves to the sockets the are accepted on this listener.
    pub fn incoming(self) -> Box<IoStream<(TcpStream, SocketAddr)>> {
        let TcpListener { loop_handle, inner } = self;
        let source = inner.source;

        inner.ready_read
            .map(move |()| {
                stream::iter(NonblockingIter { source: source.clone() }.fuse())
            })
            .flatten()
            .and_then(move |(tcp, addr)| {
                ReadinessPair::new(loop_handle.clone(), tcp).map(move |pair| {
                    let stream = TcpStream {
                        source: TcpSource(pair.source),
                        ready_read: pair.ready_read,
                        ready_write: pair.ready_write,
                    };
                    (stream, addr)
                })
            }).boxed()
    }
}

struct NonblockingIter {
    source: Arc<mio::tcp::TcpListener>,
}

impl Iterator for NonblockingIter {
    type Item = io::Result<(mio::tcp::TcpStream, SocketAddr)>;

    fn next(&mut self) -> Option<io::Result<(mio::tcp::TcpStream, SocketAddr)>> {
        match self.source.accept() {
            Ok(Some(e)) => {
                debug!("accepted connection");
                Some(Ok(e))
            }
            Ok(None) => {
                debug!("no connection ready");
                None
            }
            Err(e) => Some(Err(e)),
        }
    }
}

/// An I/O object representing a TCP stream connected to a remote endpoint.
///
/// A TCP stream can either be created by connecting to an endpoint or by
/// accepting a connection from a listener. Inside the stream is access to the
/// raw underlying I/O object as well as streams for the read/write
/// notifications on the stream itself.
#[allow(missing_docs)]
pub struct TcpStream {
    pub source: TcpSource,
    pub ready_read: ReadinessStream,
    pub ready_write: ReadinessStream,
}

#[derive(Clone)]
#[allow(missing_docs)]
pub struct TcpSource(Arc<mio::tcp::TcpStream>);

impl Read for TcpSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buf)
    }
}

impl Write for TcpSource {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

impl<'a> Read for &'a TcpSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buf)
    }
}

impl<'a> Write for &'a TcpSource {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

impl TcpStream {
    fn new(connected_stream: mio::tcp::TcpStream,
           handle: LoopHandle)
           -> Box<IoFuture<TcpStream>> {
        // Once we've connected, wait for the stream to be writable as that's
        // when the actual connection has been initiated. Once we're writable we
        // check for `take_socket_error` to see if the connect actually hit an
        // error or not.
        //
        // If all that succeeded then we ship everything on up.
        ReadinessPair::new(handle, connected_stream).and_then(|pair| {
            let ReadinessPair { source, ready_read, ready_write } = pair;
            let source_for_skip = source.clone(); // TODO: find a better way to do this
            let connected = ready_write.skip_while(move |&()| {
                match source_for_skip.take_socket_error() {
                    Ok(()) => Ok(false),
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => Ok(true),
                    Err(e) => Err(e),
                }
            });
            let connected = connected.into_future();
            connected.map(move |(_, stream)| {
                TcpStream {
                    source: TcpSource(source),
                    ready_read: ready_read,
                    ready_write: stream.into_inner()
                }
            }).map_err(|(e, _)| e)
        }).boxed()
    }

    /// Creates a new `TcpStream` from the pending socket inside the given
    /// `std::net::TcpStream`, connecting it to the address specified.
    ///
    /// This constructor allows configuring the socket before it's actually
    /// connected, and this function will transfer ownership to the returned
    /// `TcpStream` if successful. An unconnected `TcpStream` can be created
    /// with the `net2::TcpBuilder` type (and also configured via that route).
    ///
    /// The platform specific behavior of this function looks like:
    ///
    /// * On Unix, the socket is placed into nonblocking mode and then a
    ///   `connect` call is issued.
    ///
    /// * On Windows, the address is stored internally and the connect operation
    ///   is issued when the returned `TcpStream` is registered with an event
    ///   loop. Note that on Windows you must `bind` a socket before it can be
    ///   connected, so if a custom `TcpBuilder` is used it should be bound
    ///   (perhaps to `INADDR_ANY`) before this method is called.
    pub fn connect_stream(stream: net::TcpStream,
                          addr: &SocketAddr,
                          handle: LoopHandle) -> Box<IoFuture<TcpStream>> {
        match mio::tcp::TcpStream::connect_stream(stream, addr) {
            Ok(tcp) => TcpStream::new(tcp, handle),
            Err(e) => failed(e).boxed(),
        }
    }
}

impl LoopHandle {
    /// Create a new TCP listener associated with this event loop.
    ///
    /// The TCP listener will bind to the provided `addr` address, if available,
    /// and will be returned as a future. The returned future, if resolved
    /// successfully, can then be used to accept incoming connections.
    pub fn tcp_listen(self, addr: &SocketAddr) -> Box<IoFuture<TcpListener>> {
        match mio::tcp::TcpListener::bind(addr) {
            Ok(l) => TcpListener::new(l, self),
            Err(e) => failed(e).boxed(),
        }
    }

    /// Create a new TCP stream connected to the specified address.
    ///
    /// This function will create a new TCP socket and attempt to connect it to
    /// the `addr` provided. The returned future will be resolved once the
    /// stream has successfully connected. If an error happens during the
    /// connection or during the socket creation, that error will be returned to
    /// the future instead.
    pub fn tcp_connect(self, addr: &SocketAddr) -> Box<IoFuture<TcpStream>> {
        match mio::tcp::TcpStream::connect(addr) {
            Ok(tcp) => TcpStream::new(tcp, self),
            Err(e) => failed(e).boxed(),
        }
    }
}
