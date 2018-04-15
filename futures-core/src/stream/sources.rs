use std::marker::PhantomData;
use std::fmt;

use {Stream, Future, Poll, Async};
use task;

/// An empty stream.
///
/// Created by the [`empty`](::stream::empty) function.
pub struct Empty<T, E>(PhantomData<(T, E)>);

impl<T, E> Stream for Empty<T, E> {
    type Item = T;
    type Error = E;

    fn poll_next(&mut self, _cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        Ok(Async::Ready(None))
    }
}

impl<T, E> fmt::Debug for Empty<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Empty")
    }
}

impl<T, E> Default for Empty<T, E> {
    fn default() -> Self {
        Empty(PhantomData)
    }
}

impl<T, E> Clone for Empty<T, E> {
    fn clone(&self) -> Self {
        Empty(PhantomData)
    }
}

/// Creates an empty stream.
///
/// # Examples
///
/// ```
/// use futures_core::stream::*;
///
/// let empty_stream = empty::<u32, u32>();
/// ```
pub fn empty<T, E>() -> Empty<T, E> {
    Empty(PhantomData)
}

/// A stream that can produce only one item.
///
/// Created by the [`once`](::stream::once) function.
#[derive(Debug, Clone)]
pub struct Once<F: Future> {
    inner: Option<F>,
}

impl<F: Future> Stream for Once<F> {
    type Item = <F as Future>::Item;
    type Error = <F as Future>::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        let res;

        match self.inner {
            None => return Ok(Async::Ready(None)),
            Some(ref mut fut) => match fut.poll(cx) {
                Ok(Async::Pending) => return Ok(Async::Pending),
                Ok(Async::Ready(item)) => {
                    res = Ok(Async::Ready(Some(item)));
                }
                Err(err) => {
                    res = Err(err);
                }
            }
        }

        self.inner.take();
        res
    }
}

/// Creates a new stream from a future.
///
/// # Examples
///
/// ```
/// use futures_core::{future, stream::*};
///
/// let a_future = future::ok::<u32, u32>(1);
/// let a_stream = once(a_future);
/// ```
pub fn once<F: Future>(value: F) -> Once<F> {
    Once {
        inner: Some(value),
    }
}
