extern crate futures;

use std::sync::mpsc;

use futures::executor::block_on;
use futures::future::{self, FutureObj};
use futures::prelude::*;

#[test]
fn lots() {
    fn do_it(input: (i32, i32)) -> FutureObj<'static, i32> {
        let (n, x) = input;
        if n == 0 {
            FutureObj::new(Box::new(future::ready(x)))
        } else {
            FutureObj::new(Box::new(future::ready((n - 1, x + n)).then(do_it)))
        }
    }

    let (tx, rx) = mpsc::channel();
    ::std::thread::spawn(|| {
        block_on(do_it((1_000, 0)).map(move |x| tx.send(x).unwrap()))
    });
    assert_eq!(500_500, rx.recv().unwrap());
}
