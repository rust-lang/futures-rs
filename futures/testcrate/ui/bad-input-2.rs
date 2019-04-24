
#![feature(async_await, futures_api, generators)]

use futures::*;

#[async_stream]
fn foo() -> i32 {
    #[for_await(bar)]
    for i in stream::iter(vec![1, 2]) {
        stream_yield!(i);
    }
}

#[async_stream(baz)]
fn bar() -> i32 {
    #[for_await]
    for i in stream::iter(vec![1, 2]) {
        stream_yield!(i);
    }
}

fn main() {}
