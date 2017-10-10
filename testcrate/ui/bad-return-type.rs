#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;

use futures::prelude::*;

#[async]
fn foobar() -> Result<Option<i32>, ()> {
    let val = Some(42);
    if val.is_none() {
        return Ok(None)
    }
    let val = val.unwrap();
    Ok(val)
}

fn main() {}
