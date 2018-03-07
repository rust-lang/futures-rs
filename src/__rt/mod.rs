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

type StaticContext = *mut task::Context<'static>;

thread_local!(static CTX: Cell<StaticContext> = Cell::new(ptr::null_mut()));

struct Reset<'a>(StaticContext, &'a Cell<StaticContext>);

impl<'a> Reset<'a> {
    fn new(ctx: &mut task::Context, cell: &'a Cell<StaticContext>) -> Reset<'a> {
        let stored_ctx = unsafe { mem::transmute::<&mut task::Context, StaticContext>(ctx) };
        let ctx = cell.replace(stored_ctx);
        Reset(ctx, cell)
    }

    fn new_null(cell: &'a Cell<StaticContext>) -> Reset<'a> {
        let ctx = cell.replace(ptr::null_mut());
        Reset(ctx, cell)
    }
}

impl<'a> Drop for Reset<'a> {
    fn drop(&mut self) {
        self.1.set(self.0);
    }
}

pub fn in_ctx<F: FnOnce(&mut task::Context) -> T, T>(f: F) -> T {
    CTX.with(|cell| {
        let r = Reset::new_null(cell);
        if r.0 == ptr::null_mut() {
            panic!("Cannot use `await!`, `await_item!`, or `#[async] for` outside of an `async` function.")
        }
        f(unsafe { &mut *r.0 })
    })
}
