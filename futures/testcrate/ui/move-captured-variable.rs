#![feature(async_await, futures_api, generators, proc_macro_hygiene)]

use futures::*;

fn foo<F: FnMut()>(_f: F) {}

fn main() {
    let a = String::new();
    foo(|| {
        async_stream_block! {
            yield a
        };
    });
}
