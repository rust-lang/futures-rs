mod future;
mod stream;
mod pinned_future; mod pinned_stream;

use std::cell::Cell;
use std::mem;
use std::ptr;
use futures::task;

pub use self::future::*;
pub use self::stream::*;
pub use self::pinned_future::*;
pub use self::pinned_stream::*;

pub use futures::prelude::{Async, Future, Stream};
pub use stable::{StableFuture, StableStream};

pub extern crate std;
pub extern crate anchor_experiment;

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

pub struct Reset<'a>(*mut task::Context<'static>, &'a Cell<*mut task::Context<'static>>);

impl<'a> Reset<'a> {
    pub fn ctx(&mut self) -> &mut task::Context {
        if self.0 == ptr::null_mut() {
            panic!("Cannot use `await!` outside of an `async` function.")
        }
        unsafe { mem::transmute(self.0) }
    }
}

impl<'a> Drop for Reset<'a> {
    fn drop(&mut self) {
        self.1.set(self.0);
    }
}

pub fn in_ctx<F: FnOnce(Reset) -> T, T>(f: F) -> T {
    CTX.with(|cell| {
        let r = Reset(cell.get(), cell);
        f(r)
    })
}
