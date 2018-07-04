use core::mem::PinMut;

use futures_core::{Future, Poll, TryFuture};
use futures_core::task;

/// Future for the `recover` combinator, handling errors by converting them into
/// an `Item`, compatible with any error type of the caller's choosing.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Recover<A, F> {
    inner: A,
    f: Option<F>,
}

impl<A, F> Recover<A, F> {
    unsafe_pinned!(inner -> A);
}

pub fn new<A, F>(future: A, f: F) -> Recover<A, F> {
    Recover { inner: future, f: Some(f) }
}

impl<A, F> Future for Recover<A, F>
    where A: TryFuture,
          F: FnOnce(A::Error) -> A::Item,
{
    type Output = A::Item;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<A::Item> {
        self.inner().try_poll(cx)
            .map(|res| res.unwrap_or_else(|e| {
                let f = unsafe {
                    PinMut::get_mut_unchecked(self).f.take()
                        .expect("Polled future::Recover after completion")
                };
                f(e)
            }))
    }
}
