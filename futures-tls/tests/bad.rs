extern crate futures;
extern crate env_logger;
extern crate futures_mio;
extern crate futures_tls;

#[macro_use]
extern crate cfg_if;

use std::io::Error;
use std::net::ToSocketAddrs;

use futures::Future;
use futures_tls::ClientContext;

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

        use openssl::ssl;

        fn get(err: &Error) -> &openssl::error::ErrorStack {
            let err = err.get_ref().unwrap();
            match *err.downcast_ref::<ssl::error::Error>().unwrap() {
                ssl::Error::Ssl(ref v) => v,
                ref e => panic!("not an ssl eror: {:?}", e),
            }
        }

        fn verify_failed(err: &Error) {
            assert!(get(err).errors().iter().any(|e| {
                e.reason() == "certificate verify failed"
            }), "bad errors: {:?}", err);
        }

        use verify_failed as assert_expired_error;
        use verify_failed as assert_wrong_host;
        use verify_failed as assert_self_signed;
        use verify_failed as assert_untrusted_root;
    } else if #[cfg(target_os = "macos")] {
        extern crate security_framework;

        use security_framework::base::Error as SfError;

        fn assert_invalid_cert_chain(err: &Error) {
            let err = err.get_ref().unwrap();
            let err = err.downcast_ref::<SfError>().unwrap();
            assert_eq!(err.message().unwrap(), "invalid certificate chain");
        }

        use assert_invalid_cert_chain as assert_expired_error;
        use assert_invalid_cert_chain as assert_wrong_host;
        use assert_invalid_cert_chain as assert_self_signed;
        use assert_invalid_cert_chain as assert_untrusted_root;
    } else {
        extern crate winapi;

        fn assert_expired_error(err: &Error) {
            let code = err.raw_os_error().unwrap();
            assert_eq!(code as usize, winapi::CERT_E_EXPIRED as usize);
        }

        fn assert_wrong_host(err: &Error) {
            let code = err.raw_os_error().unwrap() as usize;
            // TODO: this... may be a bug in schannel-rs
            assert!(code == winapi::CERT_E_CN_NO_MATCH as usize ||
                    code == winapi::SEC_E_MESSAGE_ALTERED as usize,
                    "bad error code: {:x}", code);
        }

        fn assert_self_signed(err: &Error) {
            let code = err.raw_os_error().unwrap();
            assert_eq!(code as usize, winapi::CERT_E_UNTRUSTEDROOT as usize);
        }

        fn assert_untrusted_root(err: &Error) {
            let code = err.raw_os_error().unwrap();
            assert_eq!(code as usize, winapi::CERT_E_UNTRUSTEDROOT as usize);
        }
    }
}

fn get_host(host: &'static str) -> Error {
    drop(env_logger::init());

    let addr = format!("{}:443", host);
    let addr = t!(addr.to_socket_addrs()).next().unwrap();

    let mut l = t!(futures_mio::Loop::new());
    let client = l.handle().tcp_connect(&addr);
    let data = client.and_then(move |socket| {
        t!(ClientContext::new()).handshake(host, socket)
    });

    let res = l.run(data);
    assert!(res.is_err());
    res.err().unwrap()
}

#[test]
fn expired() {
    assert_expired_error(&get_host("expired.badssl.com"))
}

// TODO: the OSX builders on Travis apparently fail this tests spuriously?
//       passes locally though? Seems... bad!
#[test]
#[cfg_attr(all(target_os = "macos", feature = "force-openssl"), ignore)]
fn wrong_host() {
    assert_wrong_host(&get_host("wrong.host.badssl.com"))
}

#[test]
fn self_signed() {
    assert_self_signed(&get_host("self-signed.badssl.com"))
}

#[test]
fn untrusted_root() {
    assert_untrusted_root(&get_host("untrusted-root.badssl.com"))
}
