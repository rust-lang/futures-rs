use std::io::{self, ErrorKind};
use std::net::SocketAddr;

use futures::stream::Stream;
use futures::{self, Future, IntoFuture};
use mio;

use {IoFuture, IoStream, ReadinessPair, LoopHandle};

pub struct TcpListener {
    loop_handle: LoopHandle,
    inner: ReadinessPair<mio::tcp::TcpListener>,
}

impl TcpListener {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.source.local_addr()
    }

    pub fn incoming(self) -> Box<IoStream<(TcpStream, SocketAddr)>> {
        let TcpListener { loop_handle, inner } = self;
        let source = inner.source;

        inner.ready_read
            .and_then(move |()| source.accept())
            .filter_map(|i| i)
            .and_then(move |(tcp, addr)| {
                ReadinessPair::new(loop_handle.clone(), tcp).map(move |pair| {
                    (pair, addr)
                })
            }).boxed()
    }
}

pub type TcpStream = ReadinessPair<mio::tcp::TcpStream>;

impl LoopHandle {
    /// Create a new TCP listener associated with this event loop.
    ///
    /// The TCP listener will bind to the provided `addr` address, if available,
    /// and will be returned as a future. The returned future, if resolved
    /// successfully, can then be used to accept incoming connections.
    pub fn tcp_listen(self, addr: &SocketAddr) -> Box<IoFuture<TcpListener>> {
        let cloned_handle = self.clone();
        mio::tcp::TcpListener::bind(addr)
            .into_future()
            .and_then(|tcp| ReadinessPair::new(cloned_handle, tcp))
            .map(|p| TcpListener { loop_handle: self, inner: p })
            .boxed()
    }

    /// Create a new TCP stream connected to the specified address.
    ///
    /// This function will create a new TCP socket and attempt to connect it to
    /// the `addr` provided. The returned future will be resolved once the
    /// stream has successfully connected. If an error happens during the
    /// connection or during the socket creation, that error will be returned to
    /// the future instead.
    pub fn tcp_connect(self, addr: &SocketAddr) -> Box<IoFuture<TcpStream>> {
        let stream = match mio::tcp::TcpStream::connect(addr) {
            Ok(tcp) => tcp,
            Err(e) => return futures::failed(e).boxed(),
        };

        // Once we've connected, wait for the stream to be writable as that's when
        // the actual connection has been initiated. Once we're writable we check
        // for `take_socket_error` to see if the connect actually hit an error or
        // not.
        //
        // If all that succeeded then we ship everything on up.
        ReadinessPair::new(self, stream).and_then(|pair| {
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
                ReadinessPair {
                    source: source,
                    ready_read: ready_read,
                    ready_write: stream.into_inner()
                }
            })
        }).boxed()
    }
}
