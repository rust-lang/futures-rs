#![feature(proc_macro, generators)]

#[async]
fn foo() -> Result<Left, u32> {
    Err(3)
}

#[async_stream(item = Left)]
fn foos() -> Result<(), u32> {
    Err(3)
}

fn main() {}
