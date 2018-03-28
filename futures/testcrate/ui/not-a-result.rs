#![feature(proc_macro, generators, pin)]

extern crate futures;

use futures::prelude::*;

#[async_move]
fn foo() -> u32 {
    3
}

#[async_move(boxed)]
fn bar() -> u32 {
    3
}

#[async_stream_move(item = u32)]
fn foos() -> u32 {
    3
}

#[async_stream_move(boxed, item = u32)]
fn bars() -> u32 {
    3
}

fn main() {}
