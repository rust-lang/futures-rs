#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await;
extern crate futures;

use futures::Future;

#[futures_await::async]
fn foo() -> Result<i32, i32> {
    Ok(1)
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
