use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Creates a new stream where each iteration calls the provided closure
/// `F: FnMut() -> Option<T>`.
///
/// This allows creating a custom stream with any behavior
/// without using the more verbose syntax of creating a dedicated type
/// and implementing the `Stream` trait for it.
///
/// Note that the `FromFn` stream doesn’t make assumptions about the behavior of the closure,
/// and therefore conservatively does not implement [`FusedStream`](futures_core::stream::FusedStream).
///
/// The closure can use captures and its environment to track state across iterations. Depending on
/// how the stream is used, this may require specifying the `move` keyword on the closure.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
/// use futures::stream::{self, StreamExt};
///
/// let mut count = 0;
/// let stream = stream::from_fn(|| {
///     // Increment our count. This is why we started at zero.
///     count += 1;
///
///     // Check to see if we've finished counting or not.
///     if count < 6 {
///         future::ready(Some(count))
///     } else {
///         future::ready(None)
///     }
/// });
/// assert_eq!(stream.collect::<Vec<_>>().await, &[1, 2, 3, 4, 5]);
/// # });
/// ```
pub fn from_fn<F, Fut, Item>(f: F) -> FromFn<F, Fut>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<Item>>,
{
    FromFn { f, fut: None }
}

/// Stream for the [`from_fn`] function.
#[must_use = "streams do nothing unless polled"]
pub struct FromFn<F, Fut> {
    f: F,
    fut: Option<Fut>,
}

impl<F, Fut: Unpin> Unpin for FromFn<F, Fut> {}

impl<F, Fut> fmt::Debug for FromFn<F, Fut>
where
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFn").field("fut", &self.fut).finish()
    }
}

impl<F, Fut> FromFn<F, Fut> {
    unsafe_unpinned!(f: F);
    unsafe_pinned!(fut: Option<Fut>);
}

impl<F, Fut, Item> Stream for FromFn<F, Fut>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<Item>>,
{
    type Item = Item;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.fut.is_none() {
            let fut = (self.as_mut().f())();
            self.as_mut().fut().set(Some(fut));
        }

        self.as_mut()
            .fut()
            .as_pin_mut()
            .unwrap()
            .poll(cx)
            .map(|item| {
                self.as_mut().fut().set(None);
                item
            })
    }
}
