#![feature(proc_macro, proc_macro_non_items, generators, pin)]

extern crate futures;

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
