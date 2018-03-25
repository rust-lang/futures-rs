#![feature(proc_macro, conservative_impl_trait, generators, pin)]

extern crate futures_await as futures;

use futures::prelude::*;

#[async]
fn foo() -> Result<i32, i32> {
    let a: i32 = "a"; //~ ERROR: mismatched types
    Ok(1)
}

fn main() {}
