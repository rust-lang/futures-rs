use super::event_loop::LoopHandle;
use super::readiness_stream::ReadinessPair;

use mio;

use std::io::{self, ErrorKind};
use std::net::SocketAddr;

use futures::{self, Future, IntoFuture};
use futures::stream::Stream;

pub struct TcpListener {
    loop_handle: LoopHandle,
    inner: ReadinessPair<mio::tcp::TcpListener>,
}

impl TcpListener {
    pub fn new(handle: LoopHandle, addr: &SocketAddr)
               -> Box<Future<Item=TcpListener, Error=io::Error>>
    {
        let cloned_handle = handle.clone();
        mio::tcp::TcpListener::bind(addr)
            .into_future()
            .and_then(|tcp| ReadinessPair::new(cloned_handle, tcp))
            .map(|p| TcpListener { loop_handle: handle, inner: p })
            .boxed()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.source.local_addr()
    }

    pub fn incoming(self) -> Box<Stream<Item=(TcpStream, SocketAddr), Error=io::Error>> {
        let TcpListener { loop_handle, inner } = self;
        let clone_handle = move || loop_handle.clone();
        let source = inner.source;

        inner.ready_read
            .and_then(move |_| source.accept())
            .filter_map(|i| i)
            .and_then(move |(tcp, addr)| {
                ReadinessPair::new(clone_handle(), tcp)
                    .map(move |pair| (pair, addr))
            }).boxed()
    }
}

pub type TcpStream = ReadinessPair<mio::tcp::TcpStream>;

pub fn tcp_connect(handle: LoopHandle,
                   addr: &SocketAddr)
                   -> Box<Future<Item = TcpStream, Error = io::Error>> {
    match mio::tcp::TcpStream::connect(addr) {
        Ok(tcp) => {
            ReadinessPair::new(handle, tcp)
                .and_then(|ReadinessPair { source, ready_read, ready_write }| {
                    let source_for_skip = source.clone(); // TODO: find a better way to do this
                    let connected = ready_write.skip_while(move |_| {
                        match source_for_skip.take_socket_error() {
                            Ok(()) => Ok(false),
                            Err(e) => {
                                if e.kind() == ErrorKind::WouldBlock {
                                    Ok(true)
                                } else {
                                    Err(e)
                                }
                            }
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
        Err(e) => futures::failed(e).boxed(),
    }
}
