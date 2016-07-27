extern crate futures;
extern crate http;
extern crate ssl;
extern crate openssl;

use std::env;
use std::net::SocketAddr;
use std::path::Path;

use futures::*;
use http::Response;

use openssl::x509::X509;
use openssl::crypto::pkey::PKey;
use ssl::backend::openssl::ServerContextExt;

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
    })
}

fn main() {
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = t!(addr.parse::<SocketAddr>());

    println!("\n\
        Accepting connections on {addr}\n\n\
        to connect to the server run:\n\n    \
            curl --cacert {path} https://localhost:{port}/plaintext\n\n\
    ",
        addr = addr,
        port = addr.port(),
        path = Path::new(file!()).parent().unwrap().join("server.crt")
                     .display(),
    );

    http::Server::new(&addr).ssl(move || {
        let cert = include_bytes!("server.crt");
        let cert = t!(X509::from_pem(&mut &cert[..]));
        let key = include_bytes!("server.key");
        let key = t!(PKey::private_key_from_pem(&mut &key[..]));
        ssl::ServerContext::new(&cert, &key)
    }).serve(|r: http::Request| {
        assert_eq!(r.path(), "/plaintext");
        let mut r = Response::new();
        r.header("Content-Type", "text/plain; charset=UTF-8")
         .body("Hello, World!");
        finished::<_, std::io::Error>(r)
    }).unwrap()
}
