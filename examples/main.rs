#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await;
extern crate futures;

use std::io;

use futures::Future;
use futures_await::{async, await};

#[async]
fn foo() -> Result<i32, i32> {
    Ok(1)
}

#[async]
extern fn _foo1() -> Result<i32, i32> {
    Ok(1)
}

#[async]
unsafe fn _foo2() -> io::Result<i32> {
    Ok(1)
}

#[async]
unsafe extern fn _foo3() -> io::Result<i32> {
    Ok(1)
}

#[async]
pub fn _foo4() -> io::Result<i32> {
    Ok(1)
}

#[async]
fn _foo5<T>() -> Result<T, i32> {
    Err(1)
}

#[async]
fn _bar() -> Result<i32, i32> {
    await!(foo())
}

// fn foo (a: i32) -> impl ::futures_await::MyFuture<Result<i32, i32>> {
//     ::futures_await::gen((move || {
//         if false {
//             yield loop { }
//         }
//         Ok(1)
//     })())
// }

fn main() {
    println!("{:?}", foo().wait());
}
