use super::event_loop::{Direction, LoopHandle};
use super::readiness_stream::ReadinessPair;

use mio;

use std::io::{self, ErrorKind};
use std::net::SocketAddr;
use std::sync::Arc;

use futures::{self, Future, Tokens, Wake, PollError};
use futures::stream::{Stream, StreamResult};

pub struct TcpListener {
    token: usize,
    tcp: Arc<mio::tcp::TcpListener>,
    loop_handle: LoopHandle,
}

impl TcpListener {
    pub fn new(handle: LoopHandle, addr: &SocketAddr) -> io::Result<TcpListener> {
        let tcp = Arc::new(try!(mio::tcp::TcpListener::bind(addr)));
        let tok = handle.add_source(tcp.clone());
        Ok(TcpListener {
            token: tok,
            tcp: tcp,
            loop_handle: handle,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }
}

impl Stream for TcpListener {
    type Item = (TcpStream, SocketAddr);
    type Error = io::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<StreamResult<Self::Item, Self::Error>> {
        if !tokens.may_contain(&Tokens::from_usize(self.token)) {
            return None;
        }

        match self.tcp.accept() {
            Err(e) => Some(Err(PollError::Other(e))),
            Ok(Some((tcp, addr))) => {
                let pair = ReadinessPair::new(self.loop_handle.clone(), tcp);
                Some(Ok(Some((pair, addr))))
            }
            Ok(None) => None,
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.loop_handle.schedule(self.token, Direction::Read, wake)
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        self.loop_handle.drop_source(self.token)
    }
}

pub type TcpStream = ReadinessPair<mio::tcp::TcpStream>;

pub fn tcp_connect(handle: LoopHandle,
               addr: &SocketAddr)
               -> Box<Future<Item = TcpStream, Error = io::Error>> {
    match mio::tcp::TcpStream::connect(addr) {
        Ok(tcp) => {
            let ReadinessPair { source, ready_read, ready_write } =
                ReadinessPair::new(handle, tcp);
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
            }).boxed()
        }
        Err(e) => futures::failed(e).boxed(),
    }
}
