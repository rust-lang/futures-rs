#![feature(proc_macro, generators, pin)]

use futures::prelude::*;

#[async]
fn foo() -> Result<(), ()> {
}

#[async_stream(item = i32)]
fn foos() -> Result<(), ()> {
}

fn main() {}
