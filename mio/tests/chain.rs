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
        s1.write_all(b"foo ").unwrap();
        let mut s2 = TcpStream::connect(&addr).unwrap();
        s2.write_all(b"bar ").unwrap();
        let mut s3 = TcpStream::connect(&addr).unwrap();
        s3.write_all(b"baz").unwrap();
    });

    let clients = srv.incoming().map(|e| e.0).take(3).collect();
    let copied = clients.and_then(|clients| {
        let mut clients = clients.into_iter();
        let a = clients.next().unwrap().split().0;
        let b = clients.next().unwrap().split().0;
        let c = clients.next().unwrap().split().0;

        a.chain(b).chain(c).read_to_end(Vec::new())
    });

    let data = t!(l.run(copied));
    t.join().unwrap();

    assert_eq!(data, b"foo bar baz");
}
