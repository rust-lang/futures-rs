extern crate env_logger;
extern crate curl;
extern crate futures;
extern crate futures_curl;
extern crate futures_mio;

use std::cell::RefCell;
use std::sync::Arc;

use curl::easy::Easy;
use futures::Future;
use futures_mio::Loop;
use futures_curl::Session;

#[test]
fn download_rust_lang() {
    let mut lp = Loop::new().unwrap();

    let session = Session::new(lp.handle());
    let response = Arc::new(lp.add_loop_data(RefCell::new(Vec::new())));
    let headers = Arc::new(lp.add_loop_data(RefCell::new(Vec::new())));

    let mut req = Easy::new();
    req.get(true).unwrap();
    req.url("https://www.rust-lang.org").unwrap();
    let response2 = response.clone();
    req.write_function(move |data| {
        response2.get().unwrap().borrow_mut().extend_from_slice(data);
        Ok(data.len())
    }).unwrap();
    let headers2 = headers.clone();
    req.header_function(move |header| {
        headers2.get().unwrap().borrow_mut().push(header.to_vec());
        true
    }).unwrap();

    let requests = session.and_then(|sess| {
        sess.perform(req)
    }).map(move |(mut resp, err)| {
        assert!(err.is_none());
        assert_eq!(resp.response_code().unwrap(), 200);
        let response = response.get().unwrap().borrow();
        let response = String::from_utf8_lossy(&response);
        assert!(response.contains("<html>"));
        assert!(headers.get().unwrap().borrow().len() > 0);
    });

    lp.run(requests).unwrap();
}

