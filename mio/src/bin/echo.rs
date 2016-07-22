//! An echo server that just writes back everything that's written to it.

extern crate futures;
extern crate futuremio;

use std::env;
use std::net::SocketAddr;

use futures::Future;
use futures::io;
use futures::stream::Stream;

fn main() {
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let mut l = futuremio::Loop::new().unwrap();
    let server = l.handle().tcp_listen(&addr).and_then(|socket| {
        socket.incoming().for_each(|(socket, addr)| {
            let (reader, writer) = socket.split();
            io::copy(reader, writer).map(move |amt| {
                println!("wrote {} bytes to {}", amt, addr)
            }).forget();

            Ok(())
        })
    });
    println!("Listenering on: {}", addr);
    l.run(server).unwrap();
}

