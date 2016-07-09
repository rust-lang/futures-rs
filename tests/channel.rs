extern crate futures;
extern crate support;

use futures::{done, Future};
use futures::stream::*;
use support::*;

#[test]
fn sequence() {
    let (tx, mut rx) = channel();

    sassert_empty(&mut rx);
    sassert_empty(&mut rx);

    let amt = 20;
    send(amt, tx).forget();
    for i in (1..amt + 1).rev() {
        sassert_next(&mut rx, i);
    }
    sassert_done(&mut rx);

    fn send(n: u32, sender: Sender<u32, u32>)
            -> Box<Future<Item=(), Error=()>> {
        if n == 0 {
            return done(Ok(())).boxed()
        }
        sender.send(Ok(n)).map_err(|_| ()).and_then(move |sender| {
            send(n - 1, sender)
        }).boxed()
    }
}

#[test]
fn drop_sender() {
    let (tx, mut rx) = channel::<u32, u32>();
    drop(tx);
    sassert_done(&mut rx);
}
