
use futures::*;

#[async_stream] // impl Generator<Yield = Poll<U>, Return = ()>
fn _stream1() -> i32 {
    let _ = async { // impl Generator<Yield = (), Return = U>
        #[for_await]
        for i in stream::iter(vec![1, 2]) {
            future::lazy(|_| i * i).await;
        }
    };
    future::lazy(|_| ()).await;
}
