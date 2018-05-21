#![feature(test)]

extern crate futures;
extern crate futures_channel;
extern crate futures_executor;
extern crate test;

use futures::prelude::*;
use futures::future;
use futures::stream::FuturesUnordered;
use futures_channel::oneshot;
use futures_executor::block_on;

use test::Bencher;

use std::collections::VecDeque;
use std::thread;

#[bench]
fn oneshots(b: &mut Bencher) {
    const NUM: usize = 10_000;

    b.iter(|| {
        let mut txs = VecDeque::with_capacity(NUM);
        let mut rxs = FuturesUnordered::new();

        for _ in 0..NUM {
            let (tx, rx) = oneshot::channel();
            txs.push_back(tx);
            rxs.push(rx);
        }

        thread::spawn(move || {
            while let Some(tx) = txs.pop_front() {
                let _ = tx.send("hello");
            }
        });

        let f = future::poll_fn(move |cx| {
            loop {
                if let Ok(Async::Ready(None)) = rxs.poll_next(cx) {
                    break
                }
            }
            Ok::<_, ()>(Async::Ready(()))
        });
        block_on(f).unwrap();
    });
}
