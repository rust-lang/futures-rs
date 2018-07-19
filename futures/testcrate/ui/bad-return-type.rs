#![feature(use_extern_macros, proc_macro_non_items, generators, pin)]

extern crate futures;

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
        stream_yield!(None);
        return Ok(())
    }
    let val = val.unwrap();
    stream_yield!(val);
    Ok(())
}

#[async]
fn tuple() -> Result<(i32, i32), ()> {
    if false {
        return Ok(3);
    }
    Ok((1, 2))
}

fn main() {}
