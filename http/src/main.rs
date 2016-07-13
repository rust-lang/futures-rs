extern crate env_logger;
extern crate futures;
extern crate http;
extern crate time;

use std::net::SocketAddr;
use std::env;

use futures::*;
use http::Response;

fn main() {
    env_logger::init().unwrap();
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();
    http::serve(&addr, |r: http::Request| {
        assert_eq!(r.path(), "/plaintext");
        let mut r = Response::new();
        r.header("Content-Type", "text/plain")
         .body("Hello, World!");
        finished::<_, std::io::Error>(r)
    }).unwrap()
}
