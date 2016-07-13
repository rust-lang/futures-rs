extern crate futures;
extern crate futuremio;
extern crate httparse;

use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;

use futures::Future;
use futures::stream::Stream;
use futuremio::{Loop, TcpListener, TcpStream};

mod request;
pub use self::request::{Request, RequestHeaders};

mod response;
pub use self::response::Response;

mod io2;
pub use io2::{Parse, Serialize};
use io2::{ParseStream, StreamWriter};
// mod atomic;

pub trait Service<Req, Resp>: Send + Sync + 'static
    where Req: Send + 'static,
          Resp: Send + 'static
{
    type Fut: Future<Item = Resp>;

    fn process(&self, req: Req) -> Self::Fut;
}

impl<Req, Resp, Fut, F> Service<Req, Resp> for F
    where F: Fn(Req) -> Fut + Send + Sync + 'static,
          Fut: Future<Item = Resp>,
          Req: Send + 'static,
          Resp: Send + 'static
{
    type Fut = Fut;

    fn process(&self, req: Req) -> Fut {
        (self)(req)
    }
}

pub fn serve<Err, Req, Resp, S>(addr: &SocketAddr, s: S)
    where Req: Parse,
          Resp: Serialize,
          S: Service<Req, Resp>,
          <S::Fut as Future>::Error: From<Req::Error> + From<io::Error>, // TODO: simplify this?
{
    let service = Arc::new(s);
    let lp = Loop::new().unwrap();

    let listen = lp.handle().tcp_listen(addr)
        .and_then(move |listener| {
            listener.incoming().for_each(move |(stream, _)| {
                handle(stream, service.clone());
                Ok(()) // TODO: some kind of error handling
            })
        });
    lp.run(listen);
}

fn handle<Req, Resp, S>(stream: TcpStream, service: Arc<S>)
    where Req: Parse,
          Resp: Serialize,
          S: Service<Req, Resp>,
          <S::Fut as Future>::Error: From<Req::Error> + From<io::Error>,
{
    // hack around lack of Read/Write impl on Arc<...>
    let read = SourceWrapper(stream.source.clone());
    let write = SourceWrapper(stream.source);

    let input = ParseStream::new(read, stream.ready_read)
        .map_err(From::from);
    // TODO: the `and_then` here sequentializes receiving/parsing requests and
    // processing them. We want a general combiantor that let's them proceed
    // concurrently, probably up to some fixed concurrency amount.
    let responses = input.and_then(move |req| service.process(req));
    let output = StreamWriter::new(write, stream.ready_write, responses);

    output.forget()
}

// TODO: clean this up
// Hack around the lack of forwarding Read/Write impls for Arc<TcpStream>
struct SourceWrapper<S>(Arc<S>);

impl<S> Read for SourceWrapper<S>
    where for<'a> &'a S: Read
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buf)
    }
}

impl<S> Write for SourceWrapper<S>
    where for<'a> &'a S: Write
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}
