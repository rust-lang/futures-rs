extern crate env_logger;
extern crate curl;
extern crate futures;
extern crate futures_curl;
extern crate futures_mio;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use curl::easy::Easy;
use futures::Future;
use futures_mio::Loop;
use futures_curl::Session;

#[test]
fn download_rust_lang() {
    let mut lp = Loop::new().unwrap();

    let session = Session::new(lp.pin());
    let response = Arc::new(Mutex::new(Vec::new()));
    let headers = Arc::new(Mutex::new(Vec::new()));

    let mut req = Easy::new();
    req.get(true).unwrap();
    req.url("https://www.rust-lang.org").unwrap();
    let response2 = response.clone();
    req.write_function(move |data| {
        response2.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }).unwrap();
    let headers2 = headers.clone();
    req.header_function(move |header| {
        headers2.lock().unwrap().push(header.to_vec());
        true
    }).unwrap();

    let requests = session.perform(req).map(move |(mut resp, err)| {
        assert!(err.is_none());
        assert_eq!(resp.response_code().unwrap(), 200);
        let response = response.lock().unwrap();
        let response = String::from_utf8_lossy(&response);
        assert!(response.contains("<html>"));
        assert!(headers.lock().unwrap().len() > 0);
    });

    lp.run(requests).unwrap();
}

#[test]
fn timeout_download_rust_lang() {
    let mut lp = Loop::new().unwrap();

    let session = Session::new(lp.pin());

    let mut req = Easy::new();
    req.get(true).unwrap();
    req.url("https://www.rust-lang.org").unwrap();
    req.write_function(|data| Ok(data.len())).unwrap();
    let req = session.perform(req);

    let timeout = lp.handle().timeout(Duration::from_millis(5)).flatten();
    let result = req.map(Ok).select(timeout.map(Err)).then(|res| {
        match res {
            Ok((Ok((_, err)), _)) => {
                assert!(err.is_none());
                panic!("should have timed out");
            }
            Ok((Err(()), _)) => futures::finished::<(), ()>(()),
            Err((e, _)) => panic!("I/O error: {}", e),
        }
    });

    lp.run(result).unwrap();
}
