extern crate mio;

use std::net::SocketAddr;
use std::io;
use std::sync::Arc;

use super::Future;
use promise::{Promise, Cancel};
use cell::AtomicCell;

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
    inner: Arc<_TcpConnect>,
}

struct _TcpConnect {
    addr: SocketAddr,
    inner: Arc<Inner>,
    callback: AtomicCell<Option<Box<TcpConnectCallback>>>,
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
struct Callback<T> {
    cb: fn(&Callback<T>),
    data: T,
}

impl Loop {
    pub fn new() -> io::Result<Loop> {
        Ok(Loop {
            inner: INNER.with(|a| a.clone()),
        })
    }

    pub fn tcp_connect(&self, addr: &SocketAddr) -> TcpConnect {
        TcpConnect {
            inner: Arc::new(_TcpConnect {
                addr: *addr,
                inner: self.inner.clone(),
                callback: AtomicCell::new(None),
            }),
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
                // ...
            }
        }
    }
}

unsafe fn token2arc(token: mio::Token) -> Arc<Callback<()>> {
    mem::transmute(token.as_usize())
}

fn arc2token<T>(a: Arc<Callback<T>>) -> mio::Token {
    unsafe { mio::Token(mem::transmute::<_, usize>(a)) }
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
        *self.inner.callback.borrow().unwrap() = Some(Box::new(g));
        let tcp = mio::tcp::TcpStream::connect(&self.inner.addr).and_then(|tcp| {
            let poll = &self.inner.inner.poll;
            try!(poll.register(&tcp,
                               mio::Token(0),
                               mio::EventSet::writable(),
                               mio::PollOpt::edge()));
            Ok(tcp)
        });
        if let Err(e) = tcp {
            let cb = self.inner.callback.borrow().unwrap().take().unwrap();
            cb.call_box(Err(e));
        }
    }
}
