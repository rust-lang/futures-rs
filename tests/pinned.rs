#![feature(proc_macro, conservative_impl_trait, generators, underscore_lifetimes)]

extern crate futures_await as futures;

use futures::stable::PinnedFuture;
use futures::executor::current_thread;
use futures::prelude::*;

#[async]
fn foo() -> Result<i32, i32> {
    Ok(1)
}

#[async]
fn bar(x: &i32) -> Result<i32, i32> {
    Ok(*x)
}

#[async]
fn baz(x: i32) -> Result<i32, i32> {
    await!(bar(&x))
}

#[async_stream(item = u64)]
fn _stream1() -> Result<(), i32> {
    fn integer() -> u64 { 1 }
    let x = &integer();
    stream_yield!(0);
    stream_yield!(*x);
    Ok(())
}

#[async]
fn new_elision(x: &'_ i32) -> Result<i32, i32> {
    Ok(*x)
}

#[test]
fn main() {
    current_thread::run(|ctx| {
        assert_eq!(ctx.block_on(foo().anchor()), Ok(1));
        assert_eq!(ctx.block_on(bar(&1).anchor()), Ok(1));
        assert_eq!(ctx.block_on(baz(17).anchor()), Ok(17));
        assert_eq!(ctx.block_on(new_elision(&1).anchor()), Ok(1));
    })
}
