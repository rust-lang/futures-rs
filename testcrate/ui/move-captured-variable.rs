#![feature(proc_macro, conservative_impl_trait, generators, pin)]

extern crate futures_await as futures;

use futures::prelude::*;

fn foo<F: FnMut()>(_f: F) {}

fn main() {
    let a = String::new();
    foo(|| {
        async_block! {
            Ok::<String, i32>(a)
        };
    });
}
