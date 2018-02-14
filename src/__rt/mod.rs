mod future;
mod stream;

use std::cell::Cell;
use std::ptr;
use futures::task;

pub use self::future::*;
pub use self::stream::*;

pub use futures::prelude::{Async, Future, Stream};

pub extern crate std;
pub use std::ops::Generator;

#[rustc_on_unimplemented = "async functions must return a `Result` or \
                            a typedef of `Result`"]
pub trait IsResult {
    type Ok;
    type Err;

    fn into_result(self) -> Result<Self::Ok, Self::Err>;
}
impl<T, E> IsResult for Result<T, E> {
    type Ok = T;
    type Err = E;

    fn into_result(self) -> Result<Self::Ok, Self::Err> { self }
}

pub fn diverge<T>() -> T { loop {} }

/// Uninhabited type to allow `await!` to work across both `async` and
/// `async_stream`.
pub enum Mu {}

thread_local!(static CTX: Cell<*mut task::Context<'static>> = Cell::new(ptr::null_mut()));

struct Reset<'a>(*mut task::Context<'static>, &'a Cell<*mut task::Context<'static>>);

impl<'a> Drop for Reset<'a> {
    fn drop(&mut self) {
        self.1.set(self.0);
    }
}

pub fn get_ctx() -> *mut task::Context<'static> {
    CTX.with(|cell| cell.get())
}
