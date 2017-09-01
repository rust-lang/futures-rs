#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;

use futures::prelude::*;

#[async]
fn foo<T>(t: T) -> Result<T, u32> {
    Ok(t)
}

fn main() {}
