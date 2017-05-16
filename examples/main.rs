#![feature(proc_macro, conservative_impl_trait)]

extern crate futures_await;
extern crate futures;

use futures::Future;

#[futures_await::async]
fn foo(a: i32) -> Result<i32, i32> {
    ::futures::future::ok(a)
}

fn main() {
    println!("{:?}", foo(2).wait());
}
