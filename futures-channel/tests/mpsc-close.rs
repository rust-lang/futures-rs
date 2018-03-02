extern crate futures;
extern crate futures_channel;
extern crate futures_executor;

use std::thread;

use futures::prelude::*;
use futures_channel::mpsc::*;
use futures_executor::block_on;

#[test]
fn smoke() {
    let (mut sender, receiver) = channel(1);

    let t = thread::spawn(move || {
        while let Ok(s) = block_on(sender.send(42)) {
            sender = s;
        }
    });

    // `receiver` needs to be dropped for `sender` to stop sending and therefore before the join.
    drop(block_on(receiver.take(3).for_each(|_| Ok(()))).unwrap());

    t.join().unwrap()
}
