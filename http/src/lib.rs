extern crate futuremio;
extern crate net2;
#[macro_use]
extern crate futures;
extern crate httparse;
extern crate time;
#[macro_use]
extern crate log;

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;

use futures::Future;
use futures::stream::Stream;
use futuremio::{Loop, LoopHandle, TcpStream, TcpListener, IoFuture};

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

pub struct Server {
    addr: SocketAddr,
    workers: u32,
}

impl Server {
    pub fn new(addr: &SocketAddr) -> Server {
        Server {
            addr: *addr,
            workers: 1,
        }
    }

    pub fn workers(&mut self, workers: u32) -> &mut Server {
        if cfg!(unix) {
            self.workers = workers;
        }
        self
    }

    pub fn serve<Req, Resp, S>(&mut self, s: S) -> io::Result<()>
        where Req: Parse,
              Resp: Serialize,
              S: Service<Req, Resp>,
              <S::Fut as Future>::Error: From<Req::Error> + From<io::Error>, // TODO: simplify this?
    {
        let service = Arc::new(s);

        let threads = (0..self.workers - 1).map(|i| {
            let mut lp = Loop::new().unwrap();
            let service = service.clone();
            let listener = self.listener(lp.handle());
            thread::Builder::new().name(format!("worker{}", i)).spawn(move || {
                lp.run(listener.and_then(move |l| {
                    l.incoming().for_each(move |(stream, _)| {
                        handle(stream, service.clone());
                        Ok(()) // TODO: error handling
                    })
                }))
            }).unwrap()
        }).collect::<Vec<_>>();

        let mut lp = Loop::new().unwrap();
            let listener = self.listener(lp.handle());
        lp.run(listener.and_then(move |l| {
            l.incoming().for_each(move |(stream, _)| {
                handle(stream, service.clone());
                Ok(()) // TODO: error handling
            })
        })).unwrap();

        for thread in threads {
            thread.join().unwrap().unwrap();
        }

        Ok(())
    }

    fn listener(&self, handle: LoopHandle) -> Box<IoFuture<TcpListener>> {
        let listener = (|| {
            let listener = try!(net2::TcpBuilder::new_v4());
            try!(self.configure_tcp(&listener));
            try!(listener.reuse_address(true));
            try!(listener.bind(&self.addr));
            listener.listen(1024)
        })();

        match listener {
            Ok(l) => TcpListener::from_listener(l, &self.addr, handle),
            Err(e) => futures::failed(e).boxed()
        }
    }

    #[cfg(unix)]
    fn configure_tcp(&self, tcp: &net2::TcpBuilder) -> io::Result<()> {
        use net2::unix::*;

        if self.workers > 1 {
            try!(tcp.reuse_port(true));
        }

        Ok(())
    }

    #[cfg(windows)]
    fn configure_tcp(&self, _tcp: &net2::TcpBuilder) -> io::Result<()> {
        Ok(())
    }
}

fn handle<Req, Resp, S>(stream: TcpStream, service: Arc<S>)
    where Req: Parse,
          Resp: Serialize,
          S: Service<Req, Resp>,
          <S::Fut as Future>::Error: From<Req::Error> + From<io::Error>,
{
    let input = ParseStream::new(stream.source.clone(), stream.ready_read).map_err(From::from);
    let responses = input.and_then(move |req| service.process(req));
    let output = StreamWriter::new(stream.source, stream.ready_write, responses);

    // Crucially use `.forget()` here instead of returning the future, allows
    // processing multiple separate connections concurrently.
    output.forget()
}
