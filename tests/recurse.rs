extern crate futures;

use std::sync::mpsc::channel;

use futures::*;

#[test]
fn lots() {
    fn doit(n: usize) -> Box<Future<Item=(), Error=()>> {
        if n == 0 {
            finished(()).boxed()
        } else {
            finished(n - 1).and_then(doit).boxed()
        }
    }

    let (tx, rx) = channel();
    doit(1_000).map(move |_| tx.send(()).unwrap()).forget();
    rx.recv().unwrap();
}
