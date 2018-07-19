#![feature(use_extern_macros, proc_macro_non_items, generators, pin)]

extern crate futures;

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
