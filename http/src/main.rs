extern crate http;
extern crate time;
extern crate futures;

use futures::*;
use http::Response;

fn main() {
    http::serve(&"127.0.0.1:8080".parse().unwrap(), |_r| {
        let mut r = Response::new();
        r.header("Content-Type", "text/plain")
         .header("Content-Lenth", "15")
         .header("Server", "wut")
         .header("Date", &time::now().rfc822().to_string())
         .body("Hello, World!");
        finished(r).boxed()
    });
}
