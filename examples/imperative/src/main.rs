use futures::channel::mpsc;
use futures::executor; //standard executors to provide a context for futures and streams
use futures::executor::ThreadPool;
use futures::StreamExt;

fn main() {
    let pool = ThreadPool::new().expect("Failed to build pool");
    let (tx, mut rx) = mpsc::unbounded::<i32>();

    // Create a future by an async block, where async is responsible for generating
    // an implementation of Future. At this point no executor has been provided
    // to this future, so it will not be running.
    let fut_values = async {
        // Create another async block, again where Future is implemented by
        // async. Since this is inside of a parent async block, it will be
        // provided with the executor of the parent block when the parent
        // block is executed.
        //
        // This executor chaining is done by Future::poll whose second argument
        // is a std::task::Context. This represents our executor, and the Future
        // implemented by this async block can be polled using the parent async
        // block's executor.
        let fut_tx_result = async move {
            (0..100).for_each(|v| {
                tx.unbounded_send(v).expect("Failed to send");
            })
        };

        // Use the provided thread pool to spawn the transmission
        pool.spawn_ok(fut_tx_result);

        let mut pending = vec![];
        // Use the provided executor to wait for the next value
        // of the stream to be available.
        while let Some(v) = rx.next().await {
            pending.push(v * 2);
        }

        pending
    };

    // Actually execute the above future, which will invoke Future::poll and
    // subsequenty chain appropriate Future::poll and methods needing executors
    // to drive all futures. Eventually fut_values will be driven to completion.
    let values: Vec<i32> = executor::block_on(fut_values);

    println!("Values={:?}", values);
}
