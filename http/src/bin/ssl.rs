extern crate env_logger;
extern crate futures;
extern crate http;
extern crate openssl;
extern crate time;

use std::env;
use std::fs::File;
use std::net::SocketAddr;

use futures::*;
use http::Response;
use openssl::crypto::hash::Type;
use openssl::ssl::{SSL_OP_NO_SSLV2, SSL_OP_NO_SSLV3, SSL_OP_NO_COMPRESSION};
use openssl::ssl::{SslContext, SslMethod};
use openssl::x509::X509Generator;
use openssl::x509::extension::{Extension, KeyUsageOption};

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
    })
}

fn main() {
    env_logger::init().unwrap();
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = t!(addr.parse::<SocketAddr>());

    // Generate the public/private key that we're going to use.
    //
    // See docs for X509Generator
    let extension = Extension::KeyUsage(vec![KeyUsageOption::DigitalSignature]);
    let gen = X509Generator::new()
           .set_bitlength(2048)
           .set_valid_period(365*2)
           .add_name("CN".to_owned(), "localhost".to_owned())
           .set_sign_hash(Type::SHA256)
           .add_extension(extension);
    let (cert, pkey) = t!(gen.generate());

    let mut cx = t!(SslContext::new(SslMethod::Sslv23));
    cx.set_options(SSL_OP_NO_SSLV2 | SSL_OP_NO_SSLV3 | SSL_OP_NO_COMPRESSION);
    t!(cx.set_certificate(&cert));
    t!(cx.set_private_key(&pkey));
    t!(cx.set_cipher_list("ALL!EXPORT!EXPORT40!EXPORT56!aNULL!LOW!RC4@STRENGTH"));

    let path = env::temp_dir().join("futures-cert.pem");
    let mut file = t!(File::create(&path));
    t!(cert.write_pem(&mut file));

    println!("\n\
        Accepting connections on {addr}\n\n\
        to connect to the server run:\n\n    \
            curl --cacert {path} https://localhost:{port}/plaintext\n\n\
    ",
        addr = addr,
        port = addr.port(),
        path = path.display()
    );

    http::Server::new(&addr).ssl(cx).serve(|r: http::Request| {
        assert_eq!(r.path(), "/plaintext");
        let mut r = Response::new();
        r.header("Content-Type", "text/plain; charset=UTF-8")
         .body("Hello, World!");
        finished::<_, std::io::Error>(r)
    }).unwrap()
}
