#![allow(warnings)]
#![feature(proc_macro, generators)]

#[async_stream]
fn foos(a: String) -> Result<(), u32> {
    Ok(())
}

fn main() {}
