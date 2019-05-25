#![feature(async_await, generators)]

use futures::*;

#[async_stream(item = i32)]
fn foo() {
    let a: i32 = "a"; //~ ERROR: mismatched types
    yield 1;
}

fn main() {}
