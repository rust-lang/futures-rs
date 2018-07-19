#![allow(warnings)]
#![feature(use_extern_macros, proc_macro_non_items, generators, pin)]

extern crate futures;

use futures::prelude::*;

#[async_stream]
fn foos(a: String) -> Result<(), u32> {
    Ok(())
}

fn main() {}
