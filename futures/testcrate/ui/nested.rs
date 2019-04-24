#![feature(async_await, futures_api, generators)]

use futures::*;

#[async_stream]
fn _stream1() -> i32 {
    let _ = async {
        #[for_await]
        for i in stream::iter(vec![1, 2]) {
            stream_yield!(i * i);
        }
    };
}

fn main() {}
