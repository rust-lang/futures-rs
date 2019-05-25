#![feature(async_await, generators)]

use futures::*;

#[async_stream(item = ())]
fn foo() {
    yield;
    Some(())
}

fn main() {}
