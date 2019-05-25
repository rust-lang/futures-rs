#![feature(async_await, generators)]

use futures::*;

#[async_stream(item = Option<i32>)]
fn foo() -> i32 {} // ERROR

#[async_stream(item = (i32, i32))]
fn tuple() -> () {} // OK

fn main() {}
