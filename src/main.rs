#![feature(proc_macro)]

extern crate futures_await;

use futures_await::{async, await};

#[async]
fn foo(a: i32) -> i32 {
    await!(a)
}

fn main() {
    println!("{}", foo(2));
}
