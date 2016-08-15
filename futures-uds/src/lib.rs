//! Bindings for Unix Domain Sockets and futures
//!
//! This crate provides bindings between `mio_uds`, the mio crate for Unix
//! Domain sockets, and `futures`. The APIs and bindings in this crate are very
//! similar to the TCP and UDP bindings in the `futures-mio` crate. This crate
//! is also an empty crate on Windows, as Unix Domain Sockets are Unix-specific.

// NB: this is all *very* similar to TCP/UDP, and that's intentional!

#![cfg(unix)]
#![deny(missing_docs)]

extern crate futures;
extern crate futures_io;
extern crate futures_mio;
extern crate mio_uds;
#[macro_use]
extern crate log;

use std::fmt;
use std::io::{self, ErrorKind, Read, Write};
use std::mem;
use std::net::Shutdown;
use std::os::unix::net::SocketAddr;
use std::os::unix::prelude::*;
use std::path::Path;
use std::sync::Arc;

use futures::stream::{self, Stream};
use futures::{Future, Task, Poll};
use futures_io::{Ready, IoFuture, IoStream};
use futures_mio::{ReadinessStream, LoopHandle, Source};

/// A Unix socket which can accept connections from other unix sockets.
pub struct UnixListener {
    loop_handle: LoopHandle,
    ready: ReadinessStream,
    listener: Arc<Source<mio_uds::UnixListener>>,
}

impl UnixListener {
    /// Creates a new `UnixListener` bound to the specified path.
    pub fn bind<P>(path: P, handle: LoopHandle) -> IoFuture<UnixListener>
        where P: AsRef<Path>
    {
        UnixListener::_bind(path.as_ref(), handle)
    }

    fn _bind(path: &Path, handle: LoopHandle) -> IoFuture<UnixListener> {
        match mio_uds::UnixListener::bind(path) {
            Ok(s) => UnixListener::new(s, handle),
            Err(e) => futures::failed(e).boxed(),
        }
    }

    fn new(listener: mio_uds::UnixListener,
           handle: LoopHandle) -> IoFuture<UnixListener> {
        let listener = Arc::new(Source::new(listener));
        ReadinessStream::new(handle.clone(), listener.clone()).map(|r| {
            UnixListener {
                loop_handle: handle,
                ready: r,
                listener: listener,
            }
        }).boxed()
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.io().local_addr()
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.listener.io().take_error()
    }

    /// Consumes this listener, returning a stream of the sockets this listener
    /// accepts.
    ///
    /// This method returns an implementation of the `Stream` trait which
    /// resolves to the sockets the are accepted on this listener.
    pub fn incoming(self) -> IoStream<(UnixStream, SocketAddr)> {
        let UnixListener { loop_handle, listener, ready } = self;

        ready
            .map(move |_| {
                stream::iter(NonblockingIter { source: listener.clone() }.fuse())
            })
            .flatten()
            .and_then(move |(client, addr)| {
                let client = Arc::new(Source::new(client));
                ReadinessStream::new(loop_handle.clone(),
                                     client.clone()).map(move |ready| {
                    let stream = UnixStream {
                        source: client,
                        ready: ready,
                    };
                    (stream, addr)
                })
            }).boxed()
    }
}

struct NonblockingIter {
    source: Arc<Source<mio_uds::UnixListener>>,
}

impl Iterator for NonblockingIter {
    type Item = io::Result<(mio_uds::UnixStream, SocketAddr)>;

    fn next(&mut self) -> Option<io::Result<(mio_uds::UnixStream, SocketAddr)>> {
        match self.source.io().accept() {
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

impl fmt::Debug for UnixListener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.listener.io().fmt(f)
    }
}

impl Stream for UnixListener {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        self.ready.poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.ready.schedule(task)
    }
}

impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> RawFd {
        self.listener.io().as_raw_fd()
    }
}

/// A structure representing a connected unix socket.
///
/// This socket can be connected directly with `UnixStream::connect` or accepted
/// from a listener with `UnixListener::incoming`. Additionally, a pair of
/// anonymous Unix sockets can be created with `UnixStream::pair`.
pub struct UnixStream {
    source: Arc<Source<mio_uds::UnixStream>>,
    ready: ReadinessStream,
}

enum UnixStreamNew {
    Waiting(UnixStream),
    Empty,
}

impl UnixStream {
    /// Connects to the socket named by `path`.
    ///
    /// This function will create a new unix socket and connect to the path
    /// specified, performing associating the returned stream with the provided
    /// event loop's handle.
    ///
    /// The returned future will resolve once the stream is successfully
    /// connected.
    pub fn connect<P>(p: P, handle: LoopHandle) -> IoFuture<UnixStream>
        where P: AsRef<Path>
    {
        UnixStream::_connect(p.as_ref(), handle)
    }

    fn _connect(path: &Path, handle: LoopHandle) -> IoFuture<UnixStream> {
        match mio_uds::UnixStream::connect(path) {
            Ok(s) => UnixStream::new(s, handle),
            Err(e) => futures::failed(e).boxed(),
        }
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// This function will create a pair of interconnected unix sockets for
    /// communicating back and forth between one another. Each socket will be
    /// associated with the event loop whose handle is also provided.
    pub fn pair(handle: LoopHandle) -> IoFuture<(UnixStream, UnixStream)> {
        match mio_uds::UnixStream::pair() {
            Ok((a, b)) => {
                let a = UnixStream::new(a, handle.clone());
                let b = UnixStream::new(b, handle.clone());
                a.join(b).boxed()
            }
            Err(e) => futures::failed(e).boxed(),
        }
    }

    fn new(connected_stream: mio_uds::UnixStream,
           handle: LoopHandle)
           -> IoFuture<UnixStream> {
        let connected_stream = Arc::new(Source::new(connected_stream));
        ReadinessStream::new(handle, connected_stream.clone()).and_then(|ready| {
            UnixStreamNew::Waiting(UnixStream {
                source: connected_stream,
                ready: ready,
            })
        }).boxed()
    }

    /// Returns the socket address of the local half of this connection.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.source.io().local_addr()
    }

    /// Returns the socket address of the remote half of this connection.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.source.io().peer_addr()
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.source.io().take_error()
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.source.io().shutdown(how)
    }
}

impl Future for UnixStreamNew {
    type Item = UnixStream;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<UnixStream, io::Error> {
        let mut stream = match mem::replace(self, UnixStreamNew::Empty) {
            UnixStreamNew::Waiting(s) => s,
            UnixStreamNew::Empty => panic!("can't poll Unix stream twice"),
        };
        match stream.ready.poll(task) {
            Poll::Ok(None) => panic!(),
            Poll::Ok(Some(_)) => {
                match stream.source.io().take_error() {
                    Ok(None) => return Poll::Ok(stream),
                    Ok(Some(e)) => return Poll::Err(e),
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
                    Err(e) => return Poll::Err(e),
                }
            }
            Poll::Err(e) => return Poll::Err(e),
            Poll::NotReady => {}
        }
        *self = UnixStreamNew::Waiting(stream);
        Poll::NotReady
    }

    fn schedule(&mut self, task: &mut Task) {
        match *self {
            UnixStreamNew::Waiting(ref mut s) => {
                s.ready.schedule(task);
            }
            UnixStreamNew::Empty => task.notify(),
        }
    }
}

impl Read for UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let r = self.source.io().read(buf);
        trace!("read[{:p}] {:?} on {:?}", self, r, self.source.io());
        return r
    }
}

impl Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let r = self.source.io().write(buf);
        trace!("write[{:p}] {:?} on {:?}", self, r, self.source.io());
        return r
    }
    fn flush(&mut self) -> io::Result<()> {
        self.source.io().flush()
    }
}

impl<'a> Read for &'a UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.source.io().read(buf)
    }
}

impl<'a> Write for &'a UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.source.io().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.source.io().flush()
    }
}

impl fmt::Debug for UnixStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.source.io().fmt(f)
    }
}

impl Stream for UnixStream {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        self.ready.poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.ready.schedule(task)
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.source.io().as_raw_fd()
    }
}

/// An I/O object representing a Unix datagram socket.
pub struct UnixDatagram {
    source: Arc<Source<mio_uds::UnixDatagram>>,
    ready: ReadinessStream,
}

impl UnixDatagram {
    /// Creates a new `UnixDatagram` bound to the specified path.
    pub fn bind<P>(path: P, handle: LoopHandle) -> IoFuture<UnixDatagram>
        where P: AsRef<Path>
    {
        UnixDatagram::_bind(path.as_ref(), handle)
    }

    fn _bind(path: &Path, handle: LoopHandle) -> IoFuture<UnixDatagram> {
        match mio_uds::UnixDatagram::bind(path) {
            Ok(s) => UnixDatagram::new(s, handle),
            Err(e) => futures::failed(e).boxed(),
        }
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// This function will create a pair of interconnected unix sockets for
    /// communicating back and forth between one another. Each socket will be
    /// associated with the event loop whose handle is also provided.
    pub fn pair(handle: LoopHandle) -> IoFuture<(UnixDatagram, UnixDatagram)> {
        match mio_uds::UnixDatagram::pair() {
            Ok((a, b)) => {
                let a = UnixDatagram::new(a, handle.clone());
                let b = UnixDatagram::new(b, handle.clone());
                a.join(b).boxed()
            }
            Err(e) => futures::failed(e).boxed(),
        }
    }


    fn new(socket: mio_uds::UnixDatagram, handle: LoopHandle)
           -> IoFuture<UnixDatagram> {
        let socket = Arc::new(Source::new(socket));
        ReadinessStream::new(handle, socket.clone()).map(|ready| {
            UnixDatagram {
                source: socket,
                ready: ready,
            }
        }).boxed()
    }

    /// Connects the socket to the specified address.
    ///
    /// The `send` method may be used to send data to the specified address.
    /// `recv` and `recv_from` will only receive data from that address.
    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.source.io().connect(path)
    }

    /// Returns the local address that this socket is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.source.io().local_addr()
    }

    /// Returns the address of this socket's peer.
    ///
    /// The `connect` method will connect the socket to a peer.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.source.io().peer_addr()
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read and the address from
    /// whence the data came.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.source.io().recv_from(buf)
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.source.io().recv(buf)
    }

    /// Sends data on the socket to the specified address.
    ///
    /// On success, returns the number of bytes written.
    pub fn send_to<P>(&self, buf: &[u8], path: P) -> io::Result<usize>
        where P: AsRef<Path>
    {
        self.source.io().send_to(buf, path)
    }

    /// Sends data on the socket to the socket's peer.
    ///
    /// The peer address may be set by the `connect` method, and this method
    /// will return an error if the socket has not already been connected.
    ///
    /// On success, returns the number of bytes written.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.source.io().send(buf)
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.source.io().take_error()
    }

    /// Shut down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.source.io().shutdown(how)
    }
}

impl fmt::Debug for UnixDatagram {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.source.io().fmt(f)
    }
}

impl Stream for UnixDatagram {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        self.ready.poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.ready.schedule(task)
    }
}

impl AsRawFd for UnixDatagram {
    fn as_raw_fd(&self) -> RawFd {
        self.source.io().as_raw_fd()
    }
}
