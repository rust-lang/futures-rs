#![feature(async_await, generators)]

use futures::*;

#[async_stream]
fn foo() -> Left {}

fn main() {}
