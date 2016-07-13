extern crate http;
extern crate time;
extern crate futures;

use std::net::SocketAddr;
use std::env;

use futures::*;
use http::Response;

fn main() {
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();
    http::serve(&addr, |_r: http::Request| {
        let mut r = Response::new();
        r.header("Content-Type", "text/plain")
         .header("Content-Lenth", "15")
         .header("Server", "wut")
         .header("Date", &time::now().rfc822().to_string())
         .body("Hello, World!");
        finished::<_, std::io::Error>(r).boxed()
    }).unwrap()
}
