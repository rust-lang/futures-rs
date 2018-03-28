#![allow(warnings)]
#![feature(proc_macro, generators, pin)]

extern crate futures;

use futures::prelude::*;

#[async_stream]
fn foos(a: String) -> Result<(), u32> {
    Ok(())
}

fn main() {}
