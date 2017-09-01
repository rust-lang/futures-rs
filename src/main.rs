#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;

use futures::prelude::*;

#[async]
fn foo() -> u32 { //~ ERROR: wut
    3
}

// fn foo() -> impl ::futures::__rt::MyFuture<u32> {
//     (::futures::__rt::gen::<_, u32>(move || {
//         let __e: u32 = {
//             {
//                 3
//             }
//         };
//         #[allow(unreachable_code)]
//         {
//             return __e;
//             loop {
//                 yield
//             }
//         }
//     }))
// }

fn main() {}
