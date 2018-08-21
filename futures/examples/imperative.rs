#![feature(futures_api, async_await, await_macro)]
#[macro_use] extern crate futures;

use futures::{
    channel::mpsc,
    executor, //standard executors to provide a context for futures and streams
    StreamExt
};

fn main() {
    let (tx, mut rx) = mpsc::unbounded::<i32>();

    //
    // Create a future by building an async block that will be implemented as a future,
    // whose Future::poll will be called sometime later with a std::task::Context
    //
    let fut_values = async {
        //
        // Create another async block, again that implements future and will
        // be provided a std::task::Context at some point
        //
        let fut_tx_result = async {
            (0..100).for_each(|v| {
                tx.unbounded_send(v).expect("Failed to send");
            })
        };

        //
        // Use the context that has been provided to this async block by
        // Future::poll to spawn a future, which entails Future::poll
        // being invoked by some std::task::Context, whose parent is the
        // context of the current async block
        //
        spawn!(fut_tx_result);

        let mut pending = vec![];
        //
        // Await the result of rx.next(), by polling the Future using the
        // std::task::Context of this async block
        //
        while let Some(v) = await!(rx.next()) {
            pending.push(v * 2);
        };

        pending
    };

    //
    // Actually execute the above future, which will invoke Future::poll and
    // subsequenty chain appropriate Future::poll and std::task::Context's to
    // drive all futures, eventually driving the fut_values future to
    // completion.
    //
    let values: Vec<i32> = executor::block_on(fut_values);

    println!("Values={:?}", values);
}