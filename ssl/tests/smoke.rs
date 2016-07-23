extern crate futures;
extern crate env_logger;
extern crate futuremio;
extern crate openssl;
extern crate ssl;

use futures::Future;
use futures::stream::Stream;
use futures::io;
use openssl::ssl::{SslContext, SslMethod};
use openssl::ssl::{SSL_OP_NO_SSLV2, SSL_OP_NO_SSLV3, SSL_OP_NO_COMPRESSION};
use openssl::crypto::hash::Type;
use openssl::x509::X509Generator;
use openssl::x509::extension::{Extension, KeyUsageOption};
use ssl::SslStream;

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
    })
}

// returns (client, server)
fn contexts() -> (SslContext, SslContext) {
    // Generate the public/private key that we're going to use.
    //
    // See docs for X509Generator
    let extension = Extension::KeyUsage(vec![KeyUsageOption::DigitalSignature]);
    let gen = X509Generator::new()
           .set_bitlength(2048)
           .set_valid_period(365*2)
           .add_name("CN".to_owned(), "SuperMegaCorp Inc.".to_owned())
           .set_sign_hash(Type::SHA256)
           .add_extension(extension);
    let (cert, pkey) = t!(gen.generate());

    // Generate the client and server SSL contexts
    let mut cx_client = t!(SslContext::new(SslMethod::Sslv23));
    cx_client.set_options(SSL_OP_NO_SSLV2 | SSL_OP_NO_SSLV3 | SSL_OP_NO_COMPRESSION);
    t!(cx_client.set_certificate(&cert));
    t!(cx_client.set_cipher_list("ALL!EXPORT!EXPORT40!EXPORT56!aNULL!LOW!RC4@STRENGTH"));

    let mut cx_server = t!(SslContext::new(SslMethod::Sslv23));
    cx_server.set_options(SSL_OP_NO_SSLV2 | SSL_OP_NO_SSLV3 | SSL_OP_NO_COMPRESSION);
    t!(cx_server.set_certificate(&cert));
    t!(cx_server.set_private_key(&pkey));
    t!(cx_server.set_cipher_list("ALL!EXPORT!EXPORT40!EXPORT56!aNULL!LOW!RC4@STRENGTH"));

    return (cx_client, cx_server)
}

#[test]
fn client_to_server() {
    const AMT: u64 = 128 * 1024;
    drop(env_logger::init());
    let mut l = t!(futuremio::Loop::new());

    // Create a server listening on a port, then figure out what that port is
    let srv = l.handle().tcp_listen(&"127.0.0.1:0".parse().unwrap());
    let srv = t!(l.run(srv));
    let addr = t!(srv.local_addr());

    let (cx_client, cx_server) = contexts();

    // Create a future to accept one socket, connect the ssl stream, and then
    // read all the data from it.
    let socket = srv.incoming().take(1).collect();
    let received = socket.map(|mut socket| {
        socket.remove(0).0
    }).and_then(move |socket| {
        SslStream::accept(&cx_server, socket)
    }).and_then(|socket| {
        io::read_to_end(socket, Vec::new())
    });

    // Create a future to connect to our server, connect the ssl stream, and
    // then write a bunch of data to it.
    let client = l.handle().tcp_connect(&addr);
    let sent = client.and_then(move |socket| {
        SslStream::connect(&cx_client, socket)
    }).and_then(|socket| {
        io::copy(io::take(std::io::repeat(9), AMT), socket)
    });

    // Finally, run everything!
    let (amt, data) = t!(l.run(sent.join(received)));
    assert_eq!(amt, AMT);
    assert!(data == vec![9; amt as usize]);
}

#[test]
fn server_to_client() {
    const AMT: u64 = 128 * 1024;
    drop(env_logger::init());
    let mut l = t!(futuremio::Loop::new());

    // Create a server listening on a port, then figure out what that port is
    let srv = l.handle().tcp_listen(&"127.0.0.1:0".parse().unwrap());
    let srv = t!(l.run(srv));
    let addr = t!(srv.local_addr());

    let (cx_client, cx_server) = contexts();

    let socket = srv.incoming().take(1).collect();
    let sent = socket.map(|mut socket| {
        socket.remove(0).0
    }).and_then(move |socket| {
        SslStream::accept(&cx_server, socket)
    }).and_then(|socket| {
        io::copy(io::take(std::io::repeat(9), AMT), socket)
    });

    let client = l.handle().tcp_connect(&addr);
    let received = client.and_then(move |socket| {
        SslStream::connect(&cx_client, socket)
    }).and_then(|socket| {
        io::read_to_end(socket, Vec::new())
    });

    // Finally, run everything!
    let (amt, data) = t!(l.run(sent.join(received)));
    assert_eq!(amt, AMT);
    assert!(data == vec![9; amt as usize]);
}
