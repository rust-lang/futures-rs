extern crate futures;

use futures::lazy;
use futures::stream;
use futures::{Future, Sink};

#[test]
fn flatten_sink() {
    let mut v = Vec::new();
    {
        let s = lazy(|| Ok(&mut v)).flatten_sink();
        let s = s.send_all(stream::iter(vec![Ok(0), Ok(1)])).wait().unwrap();
        s.send_all(stream::iter(vec![Ok(2), Ok(3)])).wait().unwrap();
    }
    assert_eq!(v, vec![0, 1, 2, 3]);
}
