#[macro_use]
extern crate futures_io;
extern crate futures_mio;
extern crate futures_tls;
extern crate net2;
#[macro_use]
extern crate futures;
extern crate httparse;
extern crate time;
#[macro_use]
extern crate log;

use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;

use futures::{Future, Poll};
use futures::stream::Stream;
use futures_io::{IoFuture, TaskIo};
use futures_mio::{Loop, LoopHandle, TcpStream, TcpListener};
use futures_tls::{ServerContext, TlsStream};

mod request;
pub use self::request::{Request, RequestHeaders};

mod response;
pub use self::response::Response;

mod io2;
pub use io2::{Parse, Serialize};
use io2::{ParseStream, StreamWriter};

mod date;

pub trait Service<Req, Resp>: Send + Sync + 'static
    where Req: Send + 'static,
          Resp: Send + 'static
{
    type Fut: Future<Item = Resp> + Send;

    fn process(&self, req: Req) -> Self::Fut;
}

impl<Req, Resp, Fut, F> Service<Req, Resp> for F
    where F: Fn(Req) -> Fut + Send + Sync + 'static,
          Fut: Future<Item = Resp> + Send,
          Req: Send + 'static,
          Resp: Send + 'static
{
    type Fut = Fut;

    fn process(&self, req: Req) -> Fut {
        (self)(req)
    }
}

pub struct Server {
    addr: SocketAddr,
    workers: u32,
    tls: Option<Box<Fn() -> io::Result<ServerContext> + Send + Sync>>,
}

struct ServerData<S> {
    service: S,
    tls: Option<Box<Fn() -> io::Result<ServerContext> + Send + Sync>>,
}

impl Server {
    pub fn new(addr: &SocketAddr) -> Server {
        Server {
            addr: *addr,
            workers: 1,
            tls: None,
        }
    }

    pub fn workers(&mut self, workers: u32) -> &mut Server {
        if cfg!(unix) {
            self.workers = workers;
        }
        self
    }

    pub fn tls<F>(&mut self, tls: F) -> &mut Server
        where F: Fn() -> io::Result<ServerContext> + Send + Sync + 'static,
    {
        self.tls = Some(Box::new(tls));
        self
    }

    pub fn serve<Req, Resp, S>(&mut self, s: S) -> io::Result<()>
        where Req: Parse,
              Resp: Serialize,
              S: Service<Req, Resp>,
              <S::Fut as Future>::Error: From<Req::Error> + From<io::Error>, // TODO: simplify this?
    {
        let data = Arc::new(ServerData {
            service: s,
            tls: self.tls.take(),
        });

        let threads = (0..self.workers - 1).map(|i| {
            let (addr, workers) = (self.addr, self.workers);
            let data = data.clone();
            thread::Builder::new().name(format!("worker{}", i)).spawn(move || {
                let mut lp = Loop::new().unwrap();
                let listener = listener(&addr, workers, lp.handle());
                lp.run(listener.and_then(move |l| {
                    l.incoming().for_each(move |(stream, _)| {
                        handle(stream, data.clone());
                        Ok(()) // TODO: error handling
                    })
                }))
            }).unwrap()
        }).collect::<Vec<_>>();

        let mut lp = Loop::new().unwrap();
        let listener = listener(&self.addr, self.workers, lp.handle());
        lp.run(listener.and_then(move |l| {
            l.incoming().for_each(move |(stream, _)| {
                handle(stream, data.clone());
                Ok(()) // TODO: error handling
            })
        })).unwrap();

        for thread in threads {
            thread.join().unwrap().unwrap();
        }

        Ok(())
    }
}

fn listener(addr: &SocketAddr,
            workers: u32,
            handle: LoopHandle) -> IoFuture<TcpListener> {
    let listener = (|| {
        let listener = try!(net2::TcpBuilder::new_v4());
        try!(configure_tcp(workers, &listener));
        try!(listener.reuse_address(true));
        try!(listener.bind(addr));
        listener.listen(1024)
    })();

    match listener {
        Ok(l) => TcpListener::from_listener(l, addr, handle),
        Err(e) => futures::failed(e).boxed()
    }
}

#[cfg(unix)]
fn configure_tcp(workers: u32, tcp: &net2::TcpBuilder) -> io::Result<()> {
    use net2::unix::*;

    if workers > 1 {
        try!(tcp.reuse_port(true));
    }

    Ok(())
}

#[cfg(windows)]
fn configure_tcp(workers: u32, _tcp: &net2::TcpBuilder) -> io::Result<()> {
    Ok(())
}

trait IoStream: Read + Write + 'static {}

impl<T: ?Sized> IoStream for T
    where T: Read + Write + 'static
{}

fn handle<Req, Resp, S>(stream: TcpStream, data: Arc<ServerData<S>>)
    where Req: Parse,
          Resp: Serialize,
          S: Service<Req, Resp>,
          <S::Fut as Future>::Error: From<Req::Error> + From<io::Error>,
{
    let io = match data.tls {
        Some(ref tls) => {
            Either::A(tls().unwrap().handshake(stream).map(|b| {
                MaybeTls::Tls(b)
            }))
        }
        None => {
            let stream = MaybeTls::NotTls(stream);
            Either::B(futures::finished(stream))
        }
    };
    let io = io.map_err(From::from).and_then(|io| {
        let (reader, writer) = TaskIo::new(io).split();

        let input = ParseStream::new(reader).map_err(From::from);
        let responses = input.and_then(move |req| data.service.process(req));
        StreamWriter::new(writer, responses)
    });

    // Crucially use `.forget()` here instead of returning the future, allows
    // processing multiple separate connections concurrently.
    io.forget();
}

/// Temporary adapter for a read/write stream which is either TLS or not.
enum MaybeTls<S> {
    Tls(TlsStream<S>),
    NotTls(S),
}

impl<S> Read for MaybeTls<S>
    where S: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            MaybeTls::Tls(ref mut s) => s.read(buf),
            MaybeTls::NotTls(ref mut s) => s.read(buf),
        }
    }
}

impl<S> Write for MaybeTls<S>
    where S: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            MaybeTls::Tls(ref mut s) => s.write(buf),
            MaybeTls::NotTls(ref mut s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            MaybeTls::Tls(ref mut s) => s.flush(),
            MaybeTls::NotTls(ref mut s) => s.flush(),
        }
    }
}

/// Temporary adapter for a concrete type that resolves to one of two futures
enum Either<A, B> {
    A(A),
    B(B),
}

impl<A, B> Future for Either<A, B>
    where A: Future,
          B: Future<Item = A::Item, Error = A::Error>,
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self) -> Poll<A::Item, A::Error> {
        match *self {
            Either::A(ref mut s) => s.poll(),
            Either::B(ref mut s) => s.poll(),
        }
    }
}
