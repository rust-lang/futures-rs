extern crate futuremio;
extern crate futures;
extern crate httparse;
#[macro_use]
extern crate log;

use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;

use futures::Future;
use futures::stream::Stream;
use futuremio::{Loop, TcpStream};

mod request;
pub use self::request::{Request, RequestHeaders};

mod response;
pub use self::response::Response;

mod io2;
pub use io2::{Parse, Serialize};
use io2::{ParseStream, StreamWriter};

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

pub fn serve<Req, Resp, S>(addr: &SocketAddr, s: S) -> io::Result<()>
    where Req: Parse,
          Resp: Serialize,
          S: Service<Req, Resp>,
          <S::Fut as Future>::Error: From<Req::Error> + From<io::Error>, // TODO: simplify this?
{
    let service = Arc::new(s);
    let lp = Loop::new().unwrap();

    let listen = lp.handle().tcp_listen(addr)
        .map_err(From::from)
        .and_then(move |listener| {
            listener.incoming().for_each(move |(stream, _)| {
                handle(stream, service.clone());
                Ok(()) // TODO: some kind of error handling
            })
        })
        .boxed();
    lp.run(listen)
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

    let input = ParseStream::new(read, stream.ready_read).map_err(From::from);
    let responses = input.boxed().and_then(move |req| service.process(req)).boxed();
    let output = StreamWriter::new(write, stream.ready_write, responses);

    // Crucially use `.forget()` here instead of returning the future, allows
    // processing multiple separate connections concurrently.
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
