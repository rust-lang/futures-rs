use core::mem::Pin;

use futures_core::{Future, Poll};
use futures_core::task;

use FutureResult;

/// Future for the `recover` combinator, handling errors by converting them into
/// an `Item`, compatible with any error type of the caller's choosing.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Recover<A, F> {
    inner: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Recover<A, F> {
    Recover { inner: future, f: Some(f) }
}

impl<A, F> Future for Recover<A, F>
    where A: FutureResult,
          F: FnOnce(A::Error) -> A::Item,
{
    type Output = A::Item;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<A::Item> {
        unsafe { pinned_field!(self, inner) }.poll_result(cx)
            .map(|res| res.unwrap_or_else(|e| {
                let f = unsafe {
                    Pin::get_mut(&mut self).f.take()
                        .expect("Polled future::Recover after completion")
                };
                f(e)
            }))
    }
}
