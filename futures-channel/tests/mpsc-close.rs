use futures::channel::mpsc;
use futures::executor::block_on;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::thread;

#[test]
fn smoke() {
    let (mut sender, receiver) = mpsc::channel(1);

    let t = thread::spawn(move || {
        while let Ok(()) = block_on(sender.send(42)) {}
    });

    // `receiver` needs to be dropped for `sender` to stop sending and therefore before the join.
    drop(block_on(receiver.take(3).for_each(|_| futures::future::ready(()))));

    t.join().unwrap()
}
