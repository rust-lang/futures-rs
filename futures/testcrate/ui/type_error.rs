#![feature(async_await, generators)]

use futures::*;

#[async_stream]
fn foo() -> i32 {
    let a: i32 = "a"; //~ ERROR: mismatched types
    yield 1;
}

fn main() {}
