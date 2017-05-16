#![feature(proc_macro, conservative_impl_trait)]

extern crate futures_await;
extern crate futures_await_runtime;
extern crate futures;

use futures_await::async;
use futures::future;

#[async]
fn foo(a: i32) -> Result<i32, i32> {
    future::ok(a)
}

fn assert_future<F: futures::Future>(_f: F) {}

fn main() {
    assert_future(foo(2));
}
