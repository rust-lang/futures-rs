extern crate env_logger;
extern crate futures;
extern crate futures_minihttp;
extern crate time;

use std::net::SocketAddr;
use std::env;

use futures::*;
use futures_minihttp::{Server, Response, Request};

fn main() {
    env_logger::init().unwrap();
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();
    Server::new(&addr).workers(8).serve(|r: Request| {
        assert_eq!(r.path(), "/plaintext");
        let mut r = Response::new();
        r.header("Content-Type", "text/plain")
         .body("Hello, World!");
        finished::<_, ()>(r)
    }).unwrap()
}
