#![feature(async_await, generators)]

use futures::*;

#[async_stream(item = Left)]
fn foo() {}

fn main() {}
