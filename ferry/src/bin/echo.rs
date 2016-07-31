extern crate env_logger;
extern crate futures;
extern crate tokio;

use std::net::SocketAddr;
use std::env;

use tokio::Serve;
use tokio::protocols::line::{self, Request, Response};

fn main() {
    env_logger::init().unwrap();
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let service = tokio::simple_service(|r: Request| {
        Ok(Response { data: r.data.to_vec() })
    });

    line::Server::new(addr).serve(service).unwrap()
}
