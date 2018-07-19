#![feature(use_extern_macros, proc_macro_non_items, generators, pin)]

extern crate futures;

use futures::prelude::*;

#[async]
fn foo() -> Result<(), ()> {
}

#[async_stream(item = i32)]
fn foos() -> Result<(), ()> {
}

fn main() {}
