#![feature(proc_macro, generators, pin)]

extern crate futures;

use futures::prelude::*;

#[async(unpin)]
fn foo() -> u32 {
    3
}

#[async(unpin, boxed)]
fn bar() -> u32 {
    3
}

#[async_stream(unpin, item = u32)]
fn foos() -> u32 {
    3
}

#[async_stream(unpin, boxed, item = u32)]
fn bars() -> u32 {
    3
}

fn main() {}
