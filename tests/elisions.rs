#![feature(proc_macro, conservative_impl_trait, generators, underscore_lifetimes)]

extern crate futures_await as futures;

use futures::stable::PinnedFuture;
use futures::executor;
use futures::prelude::*;

struct Ref<'a, T: 'a>(&'a T);

#[async]
fn references(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async]
fn new_types(x: Ref<'_, i32>) -> Result<i32, i32> {
    Ok(*x.0)
}

#[async_move]
fn references_move(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async_stream(item = i32)]
fn _streams(x: &i32) -> Result<(), i32> {
    stream_yield!(*x);
    Ok(())
}

#[test]
fn main() {
    let x = 0;
    assert_eq!(executor::block_on(references(&x).anchor()), Ok(0));
    assert_eq!(executor::block_on(new_types(Ref(&x)).anchor()), Ok(0));
    assert_eq!(executor::block_on(references_move(&x)), Ok(0));
}
