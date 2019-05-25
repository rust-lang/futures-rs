#![feature(async_await, generators)]

use futures::*;

#[async_stream(item = i32)]
fn _stream1() {
    let _ = async {
        #[for_await]
        for i in stream::iter(vec![1, 2]) {
            yield i * i;
        }
    };
}

fn main() {}
