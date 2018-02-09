extern crate futures;
extern crate futures_channel;
extern crate futures_executor;

use std::thread;

use futures::prelude::*;
use futures_channel::mpsc::*;
use futures_executor::current_thread::run;

#[test]
fn smoke() {
    let (mut sender, receiver) = channel(1);

    let t = thread::spawn(move || {
        run(|c| {
            while let Ok(s) = c.block_on(sender.send(42)) {
                sender = s;
            }
        });
    });

    run(|c| c.block_on(receiver.take(3).for_each(|_| Ok(())))).unwrap();

    t.join().unwrap()
}
