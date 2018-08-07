#![feature(proc_macro, generators, pin)]

use futures::prelude::*;

#[async]
fn foo() -> u32 {
    3
}

#[async(boxed)]
fn bar() -> u32 {
    3
}

#[async_stream(item = u32)]
fn foos() -> u32 {
    3
}

#[async_stream(boxed, item = u32)]
fn bars() -> u32 {
    3
}

fn main() {}
