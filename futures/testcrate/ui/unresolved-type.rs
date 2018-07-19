#![feature(use_extern_macros, proc_macro_non_items, generators, pin)]

extern crate futures;

use futures::prelude::*;

#[async]
fn foo() -> Result<Left, u32> {
    Err(3)
}

#[async_stream(item = Left)]
fn foos() -> Result<(), u32> {
    Err(3)
}

fn main() {}
