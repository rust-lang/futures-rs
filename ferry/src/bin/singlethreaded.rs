extern crate env_logger;
extern crate futures;
extern crate ferry;
extern crate time;

use std::net::SocketAddr;
use std::env;

use futures::*;
use ferry::Serve;
use ferry::protocols::http::{self, Response};

fn main() {
    env_logger::init().unwrap();
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let service = ferry::simple_service(|r: http::Request| {
        assert_eq!(r.path(), "/plaintext");
        let mut r = Response::new();
        r.header("Content-Type", "text/plain; charset=UTF-8")
            .body("Hello, World!");
        finished::<_, std::io::Error>(r)
    });

    http::Server::new(addr).serve(service).unwrap()
}
