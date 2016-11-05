extern crate futures;

use std::sync::mpsc::channel;

use futures::future::{finished, Future};

#[test]
fn lots() {
    fn doit(n: usize) -> Box<Future<Item=(), Error=()> + Send> {
        if n == 0 {
            finished(()).boxed()
        } else {
            finished(n - 1).and_then(doit).boxed()
        }
    }

    let (tx, rx) = channel();
    ::std::thread::spawn(|| {
        doit(1_000).map(move |_| tx.send(()).unwrap()).wait()
    });
    rx.recv().unwrap();
}
