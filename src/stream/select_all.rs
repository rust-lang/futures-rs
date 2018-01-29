//! An unbounded set of streams
use std::fmt::{self, Debug};

use {Async, Poll, Stream};
use stream::future::StreamFuture;
use stream::futures_unordered::FuturesUnordered;

/// An unbounded set of streams
/// 
/// This "combinator" provides the ability to maintain a set of streams
/// and drive them all to completion.
/// 
/// Streams are pushed into this set and their realized values are 
/// yielded as they become ready. Streams will only be polled when they
/// generate notifications. This allows to coordinate a large number of streams.
/// 
/// Note that you can create a ready-made `SelectAll` via the
/// `select_all` function in the `stream` module, or you can start with an
/// empty set with the `SelectAll::new` constructor.
#[must_use = "streams do nothing unless polled"]
pub struct SelectAll<S> {
    inner: FuturesUnordered<StreamFuture<S>>,
}

impl<T: Debug> Debug for SelectAll<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "SelectAll {{ ... }}")
    }
}

impl<S: Stream> SelectAll<S> {
    /// Constructs a new, empty `SelectAll`
    ///
    /// The returned `SelectAll` does not contain any streams and, in this
    /// state, `SelectAll::poll` will return `Ok(Async::Ready(None))`.
    pub fn new() -> SelectAll<S> {
        SelectAll { inner: FuturesUnordered::new() }
    }

    /// Returns the number of streams contained in the set.
    ///
    /// This represents the total number of in-flight streams.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the set contains no streams
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Push a stream into the set.
    ///
    /// This function submits the given stream to the set for managing. This
    /// function will not call `poll` on the submitted stream. The caller must
    /// ensure that `SelectAll::poll` is called in order to receive task
    /// notifications.
    pub fn push(&mut self, stream: S) {
        self.inner.push(stream.into_future());
    }
}

impl<S: Stream> Stream for SelectAll<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.inner.poll().map_err(|(err, _)| err)? {
            Async::NotReady => Ok(Async::NotReady),
            Async::Ready(Some((Some(item), remaining))) => {
                self.push(remaining);
                Ok(Async::Ready(Some(item)))
            }
            Async::Ready(_) => Ok(Async::Ready(None)),
        }
    }
}
