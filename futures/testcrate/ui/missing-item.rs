#![feature(async_await, generators)]

use futures::*;

#[async_stream]
fn foo(a: String) {}

fn main() {}
