#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;

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
