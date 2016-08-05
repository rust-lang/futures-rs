extern crate curl;
extern crate hyper;

use std::env;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use hyper::{Decoder, Encoder, Next, HttpStream};
use hyper::server::{Server, Handler, Request, Response, HttpListener};
use curl::easy::Easy;

fn bin() -> PathBuf {
    env::current_exe().unwrap().parent().unwrap().join("futures-socks5")
}

static PHRASE: &'static [u8] = b"Hello World!";

struct Hello;

impl Handler<HttpStream> for Hello {
    fn on_request(&mut self, _: Request<HttpStream>) -> Next {
        Next::write()
    }
    fn on_request_readable(&mut self, _: &mut Decoder<HttpStream>) -> Next {
        Next::write()
    }
    fn on_response(&mut self, response: &mut Response) -> Next {
        use hyper::header::ContentLength;
        response.headers_mut().set(ContentLength(PHRASE.len() as u64));
        Next::write()
    }
    fn on_response_writable(&mut self, encoder: &mut Encoder<HttpStream>) -> Next {
        let n = encoder.write(PHRASE).unwrap();
        debug_assert_eq!(n, PHRASE.len());
        Next::end()
    }
}

#[test]
fn smoke() {
    let proxy_addr = "127.0.0.1:47564";
    let server_addr = "127.0.0.1:47565";
    let mut proxy = Command::new(bin()).arg(proxy_addr)
                                       .spawn()
                                       .unwrap();

    loop {
        match TcpStream::connect(proxy_addr) {
            Ok(_) => break,
            Err(_) => thread::sleep(Duration::from_millis(10)),
        }
    }

    let listener = HttpListener::bind(&server_addr.parse().unwrap()).unwrap();
    thread::spawn(move || {
        Server::new(listener).handle(|_| Hello).unwrap();
    });

    // Test socks5
    let mut handle = Easy::new();
    handle.get(true).unwrap();
    handle.url(&format!("http://{}/", server_addr)).unwrap();
    handle.proxy(&format!("socks5://{}", proxy_addr)).unwrap();
    let mut resp = Vec::new();
    {
        let mut transfer = handle.transfer();
        transfer.write_function(|data| {
            resp.extend_from_slice(data);
            Ok(data.len())
        }).unwrap();
        transfer.perform().unwrap();
    }
    assert_eq!(handle.response_code().unwrap(), 200);
    assert_eq!(resp.as_slice(), PHRASE);

    // Test socks5h
    let mut handle = Easy::new();
    handle.get(true).unwrap();
    handle.url(&format!("http://{}/", server_addr)).unwrap();
    handle.proxy(&format!("socks5h://{}", proxy_addr)).unwrap();
    let mut resp = Vec::new();
    {
        let mut transfer = handle.transfer();
        transfer.write_function(|data| {
            resp.extend_from_slice(data);
            Ok(data.len())
        }).unwrap();
        transfer.perform().unwrap();
    }
    assert_eq!(handle.response_code().unwrap(), 200);
    assert_eq!(resp.as_slice(), PHRASE);

    proxy.kill().unwrap();
    proxy.wait().unwrap();
}
