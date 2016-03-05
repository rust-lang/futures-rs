extern crate mio;
extern crate futures;

use std::net::TcpListener;
use futures::Future;

fn main() {
    let l1 = TcpListener::bind("127.0.0.1:0").unwrap();
    let l2 = TcpListener::bind("127.0.0.1:0").unwrap();
    let a1 = l1.local_addr().unwrap();
    let a2 = l2.local_addr().unwrap();
    std::thread::spawn(move || {
        let _s1 = l1.accept().unwrap();
        let _s2 = l2.accept().unwrap();
    });

    let l = futures::mio::Loop::new().unwrap();
    let s1 = l.tcp_connect(&a1);
    let s2 = l.tcp_connect(&a2);
    let c1 = s1.select(s2).await().unwrap();
}
