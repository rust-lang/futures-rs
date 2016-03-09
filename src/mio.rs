extern crate mio;

use std::net::SocketAddr;
use std::mem;
use std::io;
use std::sync::Arc;

use super::Future;
use promise::{self, Promise, Cancel, Complete};

thread_local!{
    pub static INNER: Arc<Inner> = Arc::new(Inner {
        poll: mio::Poll::new().unwrap(),
    })
}

pub struct Loop {
    inner: Arc<Inner>,
}

pub struct Inner {
    poll: mio::Poll,
}

pub struct TcpConnect {
    addr: SocketAddr,
    inner: Arc<Inner>,
}

trait TcpConnectCallback: Send + 'static {
    fn call_box(self: Box<Self>, arg: io::Result<TcpStream>);
}

impl<F> TcpConnectCallback for F
    where F: FnOnce(io::Result<TcpStream>) + Send + 'static
{
    fn call_box(self: Box<Self>, arg: io::Result<TcpStream>) {
        (*self)(arg)
    }
}

pub struct TcpStream {
    _tcp: mio::tcp::TcpStream,
    _inner: Arc<Inner>,
}

#[repr(C)]
impl Loop {
    pub fn new() -> io::Result<Loop> {
        Ok(Loop {
            inner: INNER.with(|a| a.clone()),
        })
    }

    pub fn tcp_connect(&self, addr: &SocketAddr) -> TcpConnect {
        TcpConnect {
            addr: *addr,
            inner: self.inner.clone(),
        }
    }
}

impl Inner {
    pub fn await(&self, mut promise: Promise<()>) -> Result<(), Cancel> {
        let mut events = mio::Events::new();
        loop {
            match promise.poll() {
                Ok(result) => return result,
                Err(p) => promise = p,
            }

            self.poll.poll(&mut events, None).unwrap();
            for event in events.iter() {
                let complete = unsafe {
                    token2complete(event.token())
                };
                complete.finish(event.kind());
            }
        }
    }
}

unsafe fn token2complete(token: mio::Token) -> Complete<mio::EventSet> {
    mem::transmute(token.as_usize())
}

fn complete2token(c: Complete<mio::EventSet>) -> mio::Token {
    unsafe { mio::Token(mem::transmute::<_, usize>(c)) }
}

impl Future for TcpConnect {
    type Item = TcpStream;
    type Error = io::Error;

    fn poll(self) -> Result<io::Result<TcpStream>, TcpConnect> {
        Err(self)
    }

    // fn await(self) -> io::Result<TcpStream> {
    //     let tcp = try!(mio::tcp::TcpStream::connect(&self.addr));
    //     try!(self.inner.poll.register(&tcp,
    //                                   mio::Token(0),
    //                                   mio::EventSet::writable(),
    //                                   mio::PollOpt::edge()));
    //     let mut events = mio::Events::new();
    //     'outer: loop {
    //         try!(self.inner.poll.poll(&mut events, None));
    //         for event in events.iter() {
    //             if event.token() == mio::Token(0) {
    //                 break 'outer
    //             }
    //         }
    //     }
    //     try!(self.inner.poll.deregister(&tcp));
    //
    //     Ok(TcpStream {
    //         _tcp: tcp,
    //         _inner: self.inner.clone(),
    //     })
    // }

    fn schedule<G>(self, g: G)
        where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
    {
        let tcp = match mio::tcp::TcpStream::connect(&self.addr) {
            Ok(stream) => stream,
            Err(e) => return g(Err(e)),
        };
        let (p, c) = promise::pair();
        let inner = self.inner.clone();

        // TODO: this is a race in a multithreaded event loop
        self.inner.poll.register(&tcp,
                                 complete2token(c),
                                 mio::EventSet::writable(),
                                 mio::PollOpt::edge() |
                                    mio::PollOpt::oneshot()).unwrap();
        p.schedule(move |_events| {
            g(Ok(TcpStream {
                _tcp: tcp,
                _inner: inner,
            }));
        });
    }
}
