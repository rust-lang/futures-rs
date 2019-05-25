#![feature(async_await, generators)]

use futures::*;

#[async_stream(item = Option<i32>)]
fn foobar() {
    let val = Some(42);
    if val.is_none() {
        yield None;
        return;
    }
    let val = val.unwrap();
    yield val;
}

#[async_stream(item = (i32, i32))]
fn tuple() {
    if false {
        yield 3;
    }
    yield (1, 2)
}

fn main() {}
