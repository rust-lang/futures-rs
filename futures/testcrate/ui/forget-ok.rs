#![feature(proc_macro, generators)]

#[async]
fn foo() -> Result<(), ()> {
}

#[async_stream(item = i32)]
fn foos() -> Result<(), ()> {
}

fn main() {}
