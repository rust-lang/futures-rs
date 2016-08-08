extern crate curl;

use std::env;
use std::io::{Read, Write};
use std::net::{TcpStream, TcpListener};
use std::path::PathBuf;
use std::process::{Command, Child};
use std::thread;
use std::time::Duration;

use curl::easy::Easy;

fn bin() -> PathBuf {
    env::current_exe().unwrap().parent().unwrap().join("futures-socks5")
}

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        self.0.kill().unwrap();
        self.0.wait().unwrap();
    }
}

#[test]
fn smoke() {
    let proxy_addr = "127.0.0.1:47564";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server_addr = listener.local_addr().unwrap();

    // Spawn our proxy and wait for it to come online
    let proxy = Command::new(bin()).arg(proxy_addr)
                                   .spawn()
                                   .unwrap();
    let _proxy = KillOnDrop(proxy);

    loop {
        match TcpStream::connect(proxy_addr) {
            Ok(_) => break,
            Err(_) => thread::sleep(Duration::from_millis(10)),
        }
    }

    thread::spawn(move || {
        let mut buf = [0; 1024];
        while let Ok((mut conn, _)) = listener.accept() {
            let n = conn.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.starts_with("GET / HTTP/1.1\r\n"));

            conn.write_all(b"\
                HTTP/1.1 200 OK\r\n\
                Content-Length: 13\r\n\
                \r\n\
                Hello, World!\
            ").unwrap();
        }
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
    assert_eq!(resp.as_slice(), b"Hello, World!");

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
    assert_eq!(resp.as_slice(), b"Hello, World!");
}
