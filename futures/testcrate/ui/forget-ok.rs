#![feature(proc_macro, conservative_impl_trait, generators, pin)]

extern crate futures;

use futures::prelude::*;

#[async]
fn foo() -> Result<(), ()> {
}

#[async_stream(item = i32)]
fn foos() -> Result<(), ()> {
}

fn main() {}
