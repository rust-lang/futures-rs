#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;

use futures::prelude::*;

#[async]
fn foo<T>(t: T) -> Result<T, u32> {
    Ok(t)
}

#[async_stream(item = T)]
fn foos<T>(t: T) -> Result<(), u32> {
    stream_yield!(Ok(t));
    Ok(())
}

#[async_stream(item = i32)]
fn foos2<T>(t: T) -> Result<(), u32> {
    Ok(())
}

fn main() {}
