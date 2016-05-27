extern crate futures;
extern crate futuremio;
extern crate httparse;

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::*;
use futuremio::{Loop, IoFuture, TcpListener, TcpStream};

mod request;
pub use self::request::{Request, RequestHeaders};
mod response;
pub use self::response::Response;

mod io2;
mod atomic;

pub fn serve<S>(addr: &SocketAddr, s: S)
    where S: Fn(Request) -> Box<Future<Item=Response, Error=io::Error>> +
             Sync + Send + 'static
{
    _serve(addr, Arc::new(s))
}

type Handler = Arc<Fn(Request) -> Box<IoFuture<Response>> + Send + Sync>;

fn _serve(addr: &SocketAddr, s: Handler) {
    let mut l = Loop::new().unwrap();
    let listener = l.tcp_listen(addr).unwrap();
    let f = accept(listener, s);
    l.await(f).unwrap();
}

fn accept(listener: TcpListener, cb: Handler) -> Box<IoFuture<()>> {
    let pair = listener.accept();
    pair.and_then(move |(stream, _addr)| {
        handle(stream, cb.clone())
            .join(accept(listener, cb))
            .map(|_| ())
            .boxed()
    }).boxed()
}

fn handle(stream: TcpStream, cb: Handler) -> Box<IoFuture<()>> {
    Request::new(stream)
        .and_then(move |(req, s)| cb(req).map(|resp| (resp, s)))
        .and_then(|(resp, s)| resp.send(s))
        .boxed()
}
