#![feature(async_await, futures_api, generators)]

use futures::*;

#[async_stream]
fn foobar() -> Option<i32> {
    let val = Some(42);
    if val.is_none() {
        stream_yield!(None);
        return;
    }
    let val = val.unwrap();
    stream_yield!(val);
}

#[async_stream]
fn tuple() -> (i32, i32) {
    if false {
        stream_yield!(3);
    }
    stream_yield!((1, 2))
}

fn main() {}
