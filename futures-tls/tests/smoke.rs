extern crate env_logger;
extern crate futures;
extern crate futures_io;
extern crate futures_mio;
extern crate futures_tls;

#[macro_use]
extern crate cfg_if;

use std::io::{self, Read, Write};

use futures::Future;
use futures::stream::Stream;
use futures_io::{read_to_end, copy};
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

        use openssl::crypto::hash::Type;
        use openssl::crypto::pkey::PKey;
        use openssl::crypto::rsa::RSA;
        use openssl::x509::{X509Generator, X509};

        use futures_tls::backend::openssl::ServerContextExt;
        use futures_tls::backend::openssl::ClientContextExt;

        fn server_cx() -> io::Result<ServerContext> {
            let (cert, key) = keys();
            ServerContext::new(cert, key)
        }

        fn configure_client(cx: &mut ClientContext) {
            // Unfortunately looks like the only way to configure this is
            // `set_CA_file` file on the client side so we have to actually
            // emit the certificate to a file. Do so next to our own binary
            // which is likely ephemeral as well.
            let path = t!(env::current_exe());
            let path = path.parent().unwrap().join("custom.crt");
            static INIT: Once = ONCE_INIT;
            INIT.call_once(|| {
                let pem = keys().0.to_pem().unwrap();
                t!(t!(File::create(&path)).write_all(&pem));
            });
            let ssl = cx.ssl_context_mut();
            t!(ssl.set_CA_file(&path));
        }

        // Generates a new key on the fly to be used for the entire suite of
        // tests here.
        fn keys() -> (&'static X509, &'static PKey) {
            static INIT: Once = ONCE_INIT;
            static mut CERT: *mut X509 = 0 as *mut _;
            static mut KEY: *mut PKey = 0 as *mut _;

            unsafe {
                INIT.call_once(|| {
                    let rsa = RSA::generate(1024).unwrap();
                    let pkey = PKey::from_rsa(rsa).unwrap();
                    let gen = X509Generator::new()
                                .set_valid_period(1)
                                .add_name("CN".to_owned(), "localhost".to_owned())
                                .set_sign_hash(Type::SHA256);
                    let cert = gen.sign(&pkey).unwrap();

                    CERT = Box::into_raw(Box::new(cert));
                    KEY = Box::into_raw(Box::new(pkey));
                });

                (&*CERT, &*KEY)
            }
        }
    } else if #[cfg(target_os = "macos")] {
        extern crate security_framework;

        use std::env;
        use std::fs::{self, File};
        use std::path::PathBuf;
        use std::process::Command;
        use std::sync::{Once, ONCE_INIT};

        use security_framework::certificate::SecCertificate;
        use security_framework::import_export::Pkcs12ImportOptions;
        use security_framework::keychain::SecKeychain;
        use security_framework::os::macos::import_export::Pkcs12ImportOptionsExt;
        use security_framework::os::macos::keychain::CreateOptions;
        use security_framework::os::macos::keychain::SecKeychainExt;

        use futures_tls::backend::secure_transport::ServerContextExt;
        use futures_tls::backend::secure_transport::ClientContextExt;

        fn server_cx() -> io::Result<ServerContext> {
            let (keychain, keyfile, _) = keys();
            let mut key = Vec::new();
            t!(t!(File::open(&keyfile)).read_to_end(&mut key));
            let mut keychain = t!(SecKeychain::open(&keychain));
            t!(keychain.unlock(Some("test")));

            let mut options = Pkcs12ImportOptions::new();
            Pkcs12ImportOptionsExt::keychain(&mut options, keychain);
            let identities = t!(options.passphrase("foobar")
                                       .import(&key));
            assert!(identities.len() == 1);
            ServerContext::new(&identities[0].identity, &identities[0].cert_chain)
        }

        fn configure_client(cx: &mut ClientContext) {
            let (_, _, certfile) = keys();
            let mut der = Vec::new();
            t!(t!(File::open(&certfile)).read_to_end(&mut der));
            let cert = SecCertificate::from_der(&der).unwrap();
            t!(cx.anchor_certificates(&[cert]));
        }

        // Like OpenSSL we generate certificates on the fly, but for OSX we
        // also have to put them into a specific keychain. We put both the
        // certificates and the keychain next to our binary.
        //
        // Right now I don't know of a way to programmatically create a
        // self-signed certificate, so we just fork out to the `openssl` binary.
        fn keys() -> (PathBuf, PathBuf, PathBuf) {
            static INIT: Once = ONCE_INIT;

            let path = t!(env::current_exe());
            let path = path.parent().unwrap();
            let keychain = path.join("test.keychain");
            let keyfile = path.join("test.p12");
            let certfile = path.join("test.der");

            INIT.call_once(|| {
                let _ = fs::remove_file(&keychain);
                let _ = fs::remove_file(&keyfile);
                let _ = fs::remove_file(&certfile);
                t!(CreateOptions::new()
                    .password("test")
                    .create(&keychain));

                let subj = "/C=US/ST=Denial/L=Sprintfield/O=Dis/CN=localhost";
                let output = t!(Command::new("openssl")
                                        .arg("req")
                                        .arg("-nodes")
                                        .arg("-new")
                                        .arg("-x509")
                                        .arg("-subj").arg(subj)
                                        .arg("-out").arg(&certfile)
                                        .arg("-keyout").arg(&keyfile)
                                        .output());
                assert!(output.status.success());

                let output = t!(Command::new("openssl")
                                        .arg("pkcs12")
                                        .arg("-export")
                                        .arg("-nodes")
                                        .arg("-inkey").arg(&keyfile)
                                        .arg("-in").arg(&certfile)
                                        .arg("-password").arg("pass:foobar")
                                        .output());
                assert!(output.status.success());
                t!(t!(File::create(&keyfile)).write_all(&output.stdout));

                let output = t!(Command::new("openssl")
                                        .arg("x509")
                                        .arg("-outform").arg("der")
                                        .arg("-in").arg(&certfile)
                                        .output());
                assert!(output.status.success());
                t!(t!(File::create(&certfile)).write_all(&output.stdout));
            });

            (keychain, keyfile, certfile)
        }
    } else {
        extern crate advapi32;
        extern crate crypt32;
        extern crate schannel;
        extern crate winapi;
        extern crate kernel32;

        use std::env;
        use std::io::Error;
        use std::mem;
        use std::ptr;
        use std::sync::{Once, ONCE_INIT};

        use schannel::cert_context::CertContext;
        use schannel::cert_store::{CertStore, CertAdd};

        use futures_tls::backend::schannel::ServerContextExt;

        const FRIENDLY_NAME: &'static str = "futures-tls localhost testing cert";

        fn server_cx() -> io::Result<ServerContext> {
            let mut cx = ServerContext::new();
            cx.schannel_cred().cert(localhost_cert());
            Ok(cx)
        }

        fn configure_client(cx: &mut ClientContext) {
            drop(cx);
        }

        // ====================================================================
        // Magic!
        //
        // Lots of magic is happening here to wrangle certificates for running
        // these tests on Windows. For more information see the test suite
        // in the schannel-rs crate as this is just coyping that.
        //
        // The general gist of this though is that the only way to add custom
        // trusted certificates is to add it to the system store of trust. To
        // do that we go through the whole rigamarole here to generate a new
        // self-signed certificate and then insert that into the system store.
        //
        // This generates some dialogs, so we print what we're doing sometimes,
        // and otherwise we just manage the ephemeral certificates. Because
        // they're in the system store we always ensure that they're only valid
        // for a small period of time (e.g. 1 day).

        fn localhost_cert() -> CertContext {
            static INIT: Once = ONCE_INIT;
            INIT.call_once(|| {
                for cert in local_root_store().certs() {
                    let name = match cert.friendly_name() {
                        Ok(name) => name,
                        Err(_) => continue,
                    };
                    if name != FRIENDLY_NAME {
                        continue
                    }
                    if !cert.is_time_valid().unwrap() {
                        io::stdout().write_all(br#"

The futures-tls test suite is about to delete an old copy of one of its
certificates from your root trust store. This certificate was only valid for one
day and it is no longer needed. The host should be "localhost" and the
description should mention "futures-tls".

        "#).unwrap();
                        cert.delete().unwrap();
                    } else {
                        return
                    }
                }

                install_certificate().unwrap();
            });

            for cert in local_root_store().certs() {
                let name = match cert.friendly_name() {
                    Ok(name) => name,
                    Err(_) => continue,
                };
                if name == FRIENDLY_NAME {
                    return cert
                }
            }

            panic!("couldn't find a cert");
        }

        fn local_root_store() -> CertStore {
            if env::var("APPVEYOR").is_ok() {
                CertStore::open_local_machine("Root").unwrap()
            } else {
                CertStore::open_current_user("Root").unwrap()
            }
        }

        fn install_certificate() -> io::Result<CertContext> {
            unsafe {
                let mut provider = 0;
                let mut hkey = 0;

                let mut buffer = "futures-tls test suite".encode_utf16()
                                                         .chain(Some(0))
                                                         .collect::<Vec<_>>();
                let res = advapi32::CryptAcquireContextW(&mut provider,
                                                         buffer.as_ptr(),
                                                         ptr::null_mut(),
                                                         winapi::PROV_RSA_FULL,
                                                         winapi::CRYPT_MACHINE_KEYSET);
                if res != winapi::TRUE {
                    // create a new key container (since it does not exist)
                    let res = advapi32::CryptAcquireContextW(&mut provider,
                                                             buffer.as_ptr(),
                                                             ptr::null_mut(),
                                                             winapi::PROV_RSA_FULL,
                                                             winapi::CRYPT_NEWKEYSET | winapi::CRYPT_MACHINE_KEYSET);
                    if res != winapi::TRUE {
                        return Err(Error::last_os_error())
                    }
                }

                // create a new keypair (RSA-2048)
                let res = advapi32::CryptGenKey(provider,
                                                winapi::AT_SIGNATURE,
                                                0x0800<<16 | winapi::CRYPT_EXPORTABLE,
                                                &mut hkey);
                if res != winapi::TRUE {
                    return Err(Error::last_os_error());
                }

                // start creating the certificate
                let name = "CN=localhost,O=futures-tls,OU=futures-tls,\
                            G=futures_tls".encode_utf16()
                                          .chain(Some(0))
                                          .collect::<Vec<_>>();
                let mut cname_buffer: [winapi::WCHAR; winapi::UNLEN as usize + 1] = mem::zeroed();
                let mut cname_len = cname_buffer.len() as winapi::DWORD;
                let res = crypt32::CertStrToNameW(winapi::X509_ASN_ENCODING,
                                                  name.as_ptr(),
                                                  winapi::CERT_X500_NAME_STR,
                                                  ptr::null_mut(),
                                                  cname_buffer.as_mut_ptr() as *mut u8,
                                                  &mut cname_len,
                                                  ptr::null_mut());
                if res != winapi::TRUE {
                    return Err(Error::last_os_error());
                }

                let mut subject_issuer = winapi::CERT_NAME_BLOB {
                    cbData: cname_len,
                    pbData: cname_buffer.as_ptr() as *mut u8,
                };
                let mut key_provider = winapi::CRYPT_KEY_PROV_INFO {
                    pwszContainerName: buffer.as_mut_ptr(),
                    pwszProvName: ptr::null_mut(),
                    dwProvType: winapi::PROV_RSA_FULL,
                    dwFlags: winapi::CRYPT_MACHINE_KEYSET,
                    cProvParam: 0,
                    rgProvParam: ptr::null_mut(),
                    dwKeySpec: winapi::AT_SIGNATURE,
                };
                let mut sig_algorithm = winapi::CRYPT_ALGORITHM_IDENTIFIER {
                    pszObjId: winapi::szOID_RSA_SHA256RSA.as_ptr() as *mut _,
                    Parameters: mem::zeroed(),
                };
                let mut expiration_date: winapi::SYSTEMTIME = mem::zeroed();
                kernel32::GetSystemTime(&mut expiration_date);
                let mut file_time: winapi::FILETIME = mem::zeroed();
                let res = kernel32::SystemTimeToFileTime(&mut expiration_date,
                                                         &mut file_time);
                if res != winapi::TRUE {
                    return Err(Error::last_os_error());
                }
                let mut timestamp: u64 = file_time.dwLowDateTime as u64 |
                                         (file_time.dwHighDateTime as u64) << 32;
                // one day, timestamp unit is in 100 nanosecond intervals
                timestamp += (1E9 as u64) / 100 * (60 * 60 * 24);
                file_time.dwLowDateTime = timestamp as u32;
                file_time.dwHighDateTime = (timestamp >> 32) as u32;
                let res = kernel32::FileTimeToSystemTime(&file_time,
                                                         &mut expiration_date);
                if res != winapi::TRUE {
                    return Err(Error::last_os_error());
                }

                // create a self signed certificate
                let cert_context = crypt32::CertCreateSelfSignCertificate(
                        0 as winapi::ULONG_PTR,
                        &mut subject_issuer,
                        0,
                        &mut key_provider,
                        &mut sig_algorithm,
                        ptr::null_mut(),
                        &mut expiration_date,
                        ptr::null_mut());
                if cert_context.is_null() {
                    return Err(Error::last_os_error());
                }

                // TODO: this is.. a terrible hack. Right now `schannel`
                //       doesn't provide a public method to go from a raw
                //       cert context pointer to the `CertContext` structure it
                //       has, so we just fake it here with a transmute. This'll
                //       probably break at some point, but hopefully by then
                //       it'll have a method to do this!
                struct MyCertContext<T>(T);
                impl<T> Drop for MyCertContext<T> {
                    fn drop(&mut self) {}
                }

                let cert_context = MyCertContext(cert_context);
                let cert_context: CertContext = mem::transmute(cert_context);

                try!(cert_context.set_friendly_name(FRIENDLY_NAME));

                // install the certificate to the machine's local store
                io::stdout().write_all(br#"

The futures-tls test suite is about to add a certificate to your set of root
and trusted certificates. This certificate should be for the domain "localhost"
with the description related to "futures-tls". This certificate is only valid
for one day and will be automatically deleted if you re-run the futures-tls
test suite later.

        "#).unwrap();
                try!(local_root_store().add_cert(&cert_context,
                                                 CertAdd::ReplaceExisting));
                Ok(cert_context)
            }
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
        copy(io::repeat(9).take(AMT), socket)
    });

    // Finally, run everything!
    let (amt, (_, data)) = t!(l.run(sent.join(received)));
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
        copy(io::repeat(9).take(AMT), socket)
    });

    let client = l.handle().tcp_connect(&addr);
    let received = client.and_then(move |socket| {
        t!(client_cx()).handshake("localhost", socket)
    }).and_then(|socket| {
        read_to_end(socket, Vec::new())
    });

    // Finally, run everything!
    let (amt, (_, data)) = t!(l.run(sent.join(received)));
    assert_eq!(amt, AMT);
    assert!(data == vec![9; amt as usize]);
}

struct OneByte<S> {
    inner: S,
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
        copy(io::repeat(9).take(AMT), socket)
    });

    let client = l.handle().tcp_connect(&addr);
    let received = client.and_then(move |socket| {
        t!(client_cx()).handshake("localhost", OneByte { inner: socket })
    }).and_then(|socket| {
        read_to_end(socket, Vec::new())
    });

    let (amt, (_, data)) = t!(l.run(sent.join(received)));
    assert_eq!(amt, AMT);
    assert!(data == vec![9; amt as usize]);
}
