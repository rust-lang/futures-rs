
#![feature(async_await, generators)]

use futures::*;

#[async_stream(item = i32)]
fn foo() {
    #[for_await(bar)]
    for i in stream::iter(vec![1, 2]) {
        yield i;
    }
}

#[async_stream(baz, item = i32)]
fn bar() {
    #[for_await]
    for i in stream::iter(vec![1, 2]) {
        yield i;
    }
}

fn main() {}
