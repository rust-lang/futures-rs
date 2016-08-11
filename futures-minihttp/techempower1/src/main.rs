extern crate env_logger;
extern crate futures;
extern crate futures_minihttp;
extern crate rustc_serialize;
extern crate time;

use std::net::SocketAddr;
use std::env;
use std::io;

use futures::*;
use futures_minihttp::{Request, Response, Server};

#[derive(RustcEncodable)]
struct Message {
    message: String,
}

fn main() {
    env_logger::init().unwrap();
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    Server::new(&addr).workers(8).serve(json).unwrap()
}

fn json(r: Request) -> Finished<Response, io::Error> {
    assert_eq!(r.path(), "/json");
    let msg = Message { message: "Hello, World!".to_string() };
    let mut r = Response::new();
    r.header("Content-Type", "application/json")
     .body(&rustc_serialize::json::encode(&msg).unwrap());
    finished(r)
}

