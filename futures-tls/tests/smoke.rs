extern crate env_logger;
extern crate futures;
extern crate futures_io;
extern crate futures_mio;
extern crate futures_tls;

#[macro_use]
extern crate cfg_if;

use std::io::{self, Read, Write};

use futures::{Future, Task, Poll};
use futures::stream::Stream;
use futures_io::{read_to_end, take, repeat, copy, Ready};
use futures_tls::{ServerContext, ClientContext};

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
    })
}

cfg_if! {
    if #[cfg(any(feature = "force-openssl",
                 all(not(target_os = "macos"),
                     not(target_os = "windows"))))] {
        extern crate openssl;

        use std::fs::File;
        use std::env;
        use std::sync::{Once, ONCE_INIT};

        use openssl::x509::X509;
        use openssl::crypto::pkey::PKey;

        use futures_tls::backend::openssl::ServerContextExt;
        use futures_tls::backend::openssl::ClientContextExt;

        fn server_cx() -> io::Result<ServerContext> {
            let cert = include_bytes!("server.crt");
            let cert = t!(X509::from_pem(&mut &cert[..]));
            let key = include_bytes!("server.key");
            let key = t!(PKey::private_key_from_pem(&mut &key[..]));
            ServerContext::new(&cert, &key)
        }

        static INIT: Once = ONCE_INIT;

        fn configure_client(cx: &mut ClientContext) {
            let path = t!(env::current_exe());
            let path = path.parent().unwrap().join("custom.crt");
            INIT.call_once(|| {
                t!(t!(File::create(&path)).write_all(include_bytes!("schannel-ca.crt")));
            });
            let ssl = cx.ssl_context_mut();
            t!(ssl.set_CA_file(&path));
        }
    } else if #[cfg(target_os = "macos")] {
        extern crate security_framework;

        use std::env;
        use std::fs;
        use std::sync::{Once, ONCE_INIT};

        use security_framework::certificate::SecCertificate;
        use security_framework::import_export::Pkcs12ImportOptions;
        use security_framework::keychain::SecKeychain;
        use security_framework::os::macos::import_export::Pkcs12ImportOptionsExt;
        use security_framework::os::macos::keychain::CreateOptions;
        use security_framework::os::macos::keychain::SecKeychainExt;

        use futures_tls::backend::secure_transport::ServerContextExt;
        use futures_tls::backend::secure_transport::ClientContextExt;

        static INIT: Once = ONCE_INIT;

        fn server_cx() -> io::Result<ServerContext> {
            let path = t!(env::current_exe());
            let path = path.parent().unwrap();
            let path = path.join("test.keychain");

            INIT.call_once(|| {
                let _ = fs::remove_file(&path);
                t!(CreateOptions::new()
                    .password("test")
                    .create(&path));
            });

            let mut keychain = t!(SecKeychain::open(&path));
            t!(keychain.unlock(Some("test")));

            let mut options = Pkcs12ImportOptions::new();
            Pkcs12ImportOptionsExt::keychain(&mut options, keychain);
            let identities = t!(options.passphrase("foobar")
                                       .import(include_bytes!("server.p12")));
            assert!(identities.len() == 1);
            ServerContext::new(&identities[0].identity, &identities[0].cert_chain)
        }

        fn configure_client(cx: &mut ClientContext) {
            let der = include_bytes!("server.der");
            let cert = SecCertificate::from_der(der).unwrap();
            t!(cx.anchor_certificates(&[cert]));
        }
    } else {
        extern crate schannel;

        use schannel::cert_store::CertStore;

        use futures_tls::backend::schannel::ServerContextExt;

        fn server_cx() -> io::Result<ServerContext> {
            let mut cx = ServerContext::new();
            let pkcs12 = include_bytes!("server.p12");
            let mut store = CertStore::import_pkcs12(pkcs12, "foobar").unwrap();
            let cert = store.iter().next().unwrap();
            cx.schannel_cred().cert(cert);
            Ok(cx)
        }

        fn configure_client(cx: &mut ClientContext) {
            drop(cx);
        }
    }
}

fn client_cx() -> io::Result<ClientContext> {
    let mut cx = try!(ClientContext::new());
    configure_client(&mut cx);
    Ok(cx)
}

const AMT: u64 = 128 * 1024;

#[test]
fn client_to_server() {
    drop(env_logger::init());
    let mut l = t!(futures_mio::Loop::new());

    // Create a server listening on a port, then figure out what that port is
    let srv = l.handle().tcp_listen(&"127.0.0.1:0".parse().unwrap());
    let srv = t!(l.run(srv));
    let addr = t!(srv.local_addr());

    // Create a future to accept one socket, connect the ssl stream, and then
    // read all the data from it.
    let socket = srv.incoming().take(1).collect();
    let received = socket.map(|mut socket| {
        socket.remove(0).0
    }).and_then(move |socket| {
        t!(server_cx()).handshake(socket)
    }).and_then(|socket| {
        read_to_end(socket, Vec::new())
    });

    // Create a future to connect to our server, connect the ssl stream, and
    // then write a bunch of data to it.
    let client = l.handle().tcp_connect(&addr);
    let sent = client.and_then(move |socket| {
        t!(client_cx()).handshake("localhost", socket)
    }).and_then(|socket| {
        copy(take(repeat(9), AMT), socket)
    });

    // Finally, run everything!
    let (amt, data) = t!(l.run(sent.join(received)));
    assert_eq!(amt, AMT);
    assert!(data == vec![9; amt as usize]);
}

#[test]
fn server_to_client() {
    drop(env_logger::init());
    let mut l = t!(futures_mio::Loop::new());

    // Create a server listening on a port, then figure out what that port is
    let srv = l.handle().tcp_listen(&"127.0.0.1:0".parse().unwrap());
    let srv = t!(l.run(srv));
    let addr = t!(srv.local_addr());

    let socket = srv.incoming().take(1).collect();
    let sent = socket.map(|mut socket| {
        socket.remove(0).0
    }).and_then(move |socket| {
        t!(server_cx()).handshake(socket)
    }).and_then(|socket| {
        copy(take(repeat(9), AMT), socket)
    });

    let client = l.handle().tcp_connect(&addr);
    let received = client.and_then(move |socket| {
        t!(client_cx()).handshake("localhost", socket)
    }).and_then(|socket| {
        read_to_end(socket, Vec::new())
    });

    // Finally, run everything!
    let (amt, data) = t!(l.run(sent.join(received)));
    assert_eq!(amt, AMT);
    assert!(data == vec![9; amt as usize]);
}

struct OneByte<S> {
    inner: S,
}

impl<S> Stream for OneByte<S>
    where S: Stream<Item=Ready, Error=io::Error>
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        self.inner.poll(task)
    }

    fn schedule(&mut self, task: &mut Task) {
        self.inner.schedule(task)
    }
}

impl<S: Read> Read for OneByte<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(&mut buf[..1])
    }
}

impl<S: Write> Write for OneByte<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(&buf[..1])
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[test]
fn one_byte_at_a_time() {
    drop(env_logger::init());
    let mut l = t!(futures_mio::Loop::new());

    let srv = l.handle().tcp_listen(&"127.0.0.1:0".parse().unwrap());
    let srv = t!(l.run(srv));
    let addr = t!(srv.local_addr());

    let socket = srv.incoming().take(1).collect();
    let sent = socket.map(|mut socket| {
        socket.remove(0).0
    }).and_then(move |socket| {
        t!(server_cx()).handshake(OneByte { inner: socket })
    }).and_then(|socket| {
        copy(take(repeat(9), AMT), socket)
    });

    let client = l.handle().tcp_connect(&addr);
    let received = client.and_then(move |socket| {
        t!(client_cx()).handshake("localhost", OneByte { inner: socket })
    }).and_then(|socket| {
        read_to_end(socket, Vec::new())
    });

    let (amt, data) = t!(l.run(sent.join(received)));
    assert_eq!(amt, AMT);
    assert!(data == vec![9; amt as usize]);
}
