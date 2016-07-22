extern crate futures;
extern crate futuremio;

use std::net::TcpStream;
use std::thread;
use std::io::Write;

use futures::Future;
use futures::stream::Stream;
use futures::io::ReadStream;

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
    })
}

#[test]
fn chain() {
    let mut l = t!(futuremio::Loop::new());
    let srv = l.handle().tcp_listen(&"127.0.0.1:0".parse().unwrap());
    let srv = t!(l.run(srv));
    let addr = t!(srv.local_addr());

    let t = thread::spawn(move || {
        let mut s1 = TcpStream::connect(&addr).unwrap();
        s1.write_all(b"foo bar baz").unwrap();
    });

    let clients = srv.incoming().map(|e| e.0).take(1).collect();
    let copied = clients.and_then(|clients| {
        let mut clients = clients.into_iter();
        let a = clients.next().unwrap().split().0;

        a.limit(4).read_to_end(Vec::new())
    });

    let data = t!(l.run(copied));
    t.join().unwrap();

    assert_eq!(data, b"foo ");
}
