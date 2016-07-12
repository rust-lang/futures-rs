extern crate mio;
extern crate futures;
extern crate slab;

#[macro_use]
extern crate scoped_tls;

use std::io;
use futures::Future;
use futures::stream::Stream;

mod readiness_stream;
mod event_loop;
mod tcp;

pub type IoFuture<T> = Future<Item=T, Error=io::Error>;
pub type IoStream<T> = Stream<Item=T, Error=io::Error>;

pub use event_loop::{Loop, LoopHandle, Direction};
pub use readiness_stream::{ReadinessStream, ReadinessPair};
pub use tcp::{TcpListener, TcpStream, tcp_connect};
