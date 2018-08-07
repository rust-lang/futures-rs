#![allow(warnings)]
#![feature(proc_macro, generators, pin)]

#[async_stream]
fn foos(a: String) -> Result<(), u32> {
    Ok(())
}

fn main() {}
