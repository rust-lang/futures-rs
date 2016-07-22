//! A small server that writes as many nul bytes on all connections it receives.
//!
//! There is no concurrency in this server, only one connection is written to at
//! a time.

extern crate futures;
extern crate futuremio;

use std::env;
use std::io::Write;
use std::net::SocketAddr;

use futures::Future;
use futures::stream::Stream;

fn main() {
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let mut l = futuremio::Loop::new().unwrap();
    let server = l.handle().tcp_listen(&addr).and_then(|socket| {
        socket.incoming().and_then(|(socket, addr)| {
            println!("got a socket: {}", addr);
            let buf = [0; 64 * 1024];
            let mut source = socket.source;
            socket.ready_write.for_each(move |()| {
                while let Ok(_) = source.write(&buf) {}
                Ok(())
            })
        }).for_each(|()| {
            println!("lost the socket");
            Ok(())
        })
    });
    println!("Listenering on: {}", addr);
    l.run(server).unwrap();
}
