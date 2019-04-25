#![feature(async_await, generators)]

use futures::*;

#[async_stream]
fn foobar() -> Option<i32> {
    let val = Some(42);
    if val.is_none() {
        yield None;
        return;
    }
    let val = val.unwrap();
    yield val;
}

#[async_stream]
fn tuple() -> (i32, i32) {
    if false {
        yield 3;
    }
    yield (1, 2)
}

fn main() {}
