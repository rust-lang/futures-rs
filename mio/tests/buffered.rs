extern crate futures;
extern crate futuremio;
extern crate env_logger;

use std::net::{TcpStream, Shutdown};
use std::thread;
use std::io::{Read, Write};

use futures::Future;
use futures::stream::Stream;
use futures::io;

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
    })
}

#[test]
fn echo_server() {
    drop(env_logger::init());

    let mut l = t!(futuremio::Loop::new());
    let srv = l.handle().tcp_listen(&"127.0.0.1:0".parse().unwrap());
    let srv = t!(l.run(srv));
    let addr = t!(srv.local_addr());

    let msg = "foo bar baz";
    let t = thread::spawn(move || {
        let mut s = t!(TcpStream::connect(&addr));
        let mut s2 = t!(s.try_clone());

        let t = thread::spawn(move || {
            let mut b = Vec::new();
            t!(s2.read_to_end(&mut b));
            b
        });

        let mut expected = Vec::<u8>::new();
        for _i in 0..1024 {
            expected.extend(msg.as_bytes());
            assert_eq!(t!(s.write(msg.as_bytes())), msg.len());
        }
        t!(s.shutdown(Shutdown::Write));

        let actual = t.join().unwrap();
        assert!(actual == expected);
    });

    let clients = srv.incoming();
    let client = clients.into_future().map(|e| e.0.unwrap()).map_err(|e| e.0);
    let halves = client.map(|s| s.0.split());
    let copied = halves.and_then(|(a, b)| {
        let a = io::BufReader::new(a);
        let b = io::BufWriter::new(b);
        io::copy(a, b)
    });

    let amt = t!(l.run(copied));
    t.join().unwrap();

    assert_eq!(amt, msg.len() as u64 * 1024);
}
