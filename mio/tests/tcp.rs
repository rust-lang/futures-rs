extern crate futures;
extern crate futuremio;

use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::thread;

use futures::Future;

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
    })
}

#[test]
fn connect() {
    let mut l = t!(futuremio::Loop::new());
    let srv = t!(TcpListener::bind("127.0.0.1:0"));
    let addr = t!(srv.local_addr());
    let t = thread::spawn(move || {
        t!(srv.accept()).0
    });

    let stream = l.tcp_connect(&addr);
    let mine = t!(l.await(stream));
    let theirs = t.join().unwrap();

    assert_eq!(t!(mine.local_addr()), t!(theirs.peer_addr()));
    assert_eq!(t!(theirs.local_addr()), t!(mine.peer_addr()));
}

#[test]
fn accept() {
    let mut l = t!(futuremio::Loop::new());
    let srv = t!(l.tcp_listen(&"127.0.0.1:0".parse().unwrap()));
    let addr = t!(srv.local_addr());

    let (tx, rx) = channel();
    let client = srv.accept().map(move |t| {
        tx.send(()).unwrap();
        t.0
    });
    assert!(rx.try_recv().is_err());
    let t = thread::spawn(move || {
        TcpStream::connect(&addr).unwrap()
    });

    let mine = t!(l.await(client));
    let theirs = t.join().unwrap();

    assert_eq!(t!(mine.local_addr()), t!(theirs.peer_addr()));
    assert_eq!(t!(theirs.local_addr()), t!(mine.peer_addr()));
}

#[test]
fn accept2() {
    let mut l = t!(futuremio::Loop::new());
    let srv = t!(l.tcp_listen(&"127.0.0.1:0".parse().unwrap()));
    let addr = t!(srv.local_addr());
    let t = thread::spawn(move || {
        TcpStream::connect(&addr).unwrap()
    });

    t!(l.await(srv.accept().map(|t| t.0)));
    t.join().unwrap();
}
