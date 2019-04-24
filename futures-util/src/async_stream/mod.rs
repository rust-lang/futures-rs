//! Await
//!
//! This module contains a number of functions and combinators for working
//! with `async`/`await` code.

#[macro_use]
mod stream_yield;
pub use self::stream_yield::*;

use futures_core::future::Future;
use futures_core::stream::Stream;
use std::future::{self, get_task_context, set_task_context};
use std::marker::PhantomData;
use std::ops::{Generator, GeneratorState};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Wrap a generator in a stream.
///
/// This function returns a `GenStream` underneath, but hides it in `impl Trait` to give
/// better error messages (`impl Stream` rather than `GenStream<[closure.....]>`).
pub fn from_generator<U, T>(x: T) -> impl Stream<Item = U>
where
    T: Generator<Yield = Poll<U>, Return = ()>,
{
    GenStream { gen: x, done: false, _phantom: PhantomData }
}

/// A wrapper around generators used to implement `Stream` for `async`/`await` code.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct GenStream<U, T: Generator<Yield = Poll<U>, Return = ()>> {
    gen: T,
    // If resuming after Complete, std generators panic. This is natural when using the generators,
    // but in the streams, we may want to call `poll_next` after `poll_next` returns `None`
    // because it is possible to call `next` after `next` returns `None` in many iterators.
    done: bool,
    _phantom: PhantomData<U>,
}

impl<U, T: Generator<Yield = Poll<U>, Return = ()>> Stream for GenStream<U, T> {
    type Item = U;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }
        // Safe because we're !Unpin + !Drop mapping to a ?Unpin value
        let Self { gen, done, .. } = unsafe { Pin::get_unchecked_mut(self) };
        let gen = unsafe { Pin::new_unchecked(gen) };
        set_task_context(cx, || match gen.resume() {
            GeneratorState::Yielded(x) => x.map(Some),
            GeneratorState::Complete(()) => {
                *done = true;
                Poll::Ready(None)
            }
        })
    }
}

/// Polls a stream in the current thread-local task waker.
pub fn poll_next_with_tls_context<S>(s: Pin<&mut S>) -> Poll<Option<S::Item>>
where
    S: Stream,
{
    get_task_context(|cx| S::poll_next(s, cx))
}

// The `await!` called in `async_stream` needs to be adjust to yield `Poll`,
// but for this purpose, we don't want the user to use `#![feature(gen_future)]`.
/// Polls a future in the current thread-local task waker.
#[inline]
pub fn poll_with_tls_context<F>(f: Pin<&mut F>) -> Poll<F::Output>
where
    F: Future
{
    future::poll_with_tls_context(f)
}
