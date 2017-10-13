#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;

use futures::prelude::*;

#[async]
fn foo() -> Result<(), ()> {
}

#[async_stream(item = i32)]
fn foos() -> Result<(), ()> {
}

fn main() {}
