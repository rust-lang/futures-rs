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

#[async_stream(item = Option<i32>)]
fn foobars() -> Result<(), ()> {
    let val = Some(42);
    if val.is_none() {
        stream_yield!(Ok(None));
        return Ok(())
    }
    let val = val.unwrap();
    stream_yield!(Ok(val));
    Ok(())
}

fn main() {}
