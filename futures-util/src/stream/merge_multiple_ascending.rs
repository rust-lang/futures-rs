use std::pin::Pin;
use std::task::{Context, Poll};
use Poll::*;

use crate::stream::{Fuse, StreamExt};
use futures_core::stream::{FusedStream, Stream};
use pin_project_lite::pin_project;

/// Merge multiple ordered streams in constant space. The precondition (input
/// streams are ascending) is not checked.
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
/// let s1 = stream::iter(0..10);
/// let s2 = stream::iter(10..20);
/// let s3 = stream::iter(20..30);
/// let collected: Vec<i32> = stream::merge_multiple_ascending([s1, s2, s3]).collect().await;
/// assert_eq!(collected, (0..30).collect::<Vec<i32>>());
/// # });
/// ```
///
/// NOTE: this is not as easy to use as [`merge_ascending`](crate::stream::merge_asceding). Every stream in the
/// iterator must be `Unpin` and have _exactly_ the same type as opposed to the
/// two stream case where both streams need only implement the `Stream<Item =
/// T>` trait. In practice, you will likely need to Box your streams into a
/// `dyn` trait object if you want to use this function.
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::Future;
/// use futures::stream::{self, StreamExt};
/// use std::pin::Pin;
/// type BoxedUnfoldGenerator = Box<dyn Fn(i32) -> Pin<Box<dyn Future<Output = Option<(i32, i32)>>>>>;
/// let f1: BoxedUnfoldGenerator = Box::new(|state: i32| {
///     Box::pin(async move {
///         if state >= 5 {
///             None
///         } else {
///             Some((state * 3, state + 1))
///         }
///     })
/// });
/// let f2: BoxedUnfoldGenerator = Box::new(|state: i32| {
///     Box::pin(async move {
///         if state >= 5 {
///             None
///         } else {
///             Some((state * 3 + 1, state + 1))
///         }
///     })
/// });
/// let f3: BoxedUnfoldGenerator = Box::new(|state: i32| {
///     Box::pin(async move {
///         if state >= 5 {
///             None
///         } else {
///             Some((state * 3 + 2, state + 1))
///         }
///     })
/// });
/// let s1 = stream::unfold(0, f1);
/// let s2 = stream::unfold(0, f2);
/// let s3 = stream::unfold(0, f3);
/// let collected: Vec<i32> = stream::merge_multiple_ascending([s1, s2, s3]).collect().await;
/// assert_eq!(collected, (0..15).collect::<Vec<i32>>());
/// # });
/// ```
pub fn merge_multiple_ascending<T: Ord, St: Stream<Item = T>>(
    streams: impl IntoIterator<Item = St>,
) -> MergeMultipleAscending<T, St> {
    let stream_vec: Vec<_> = streams.into_iter().map(|s| s.fuse()).collect();
    let n = stream_vec.len();
    let mut peeks = Vec::with_capacity(n);
    peeks.resize_with(n, || None);
    MergeMultipleAscending { streams: stream_vec, peeks }
}

pin_project! {
    /// Struct for the `merge_multiple_ascending` method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct MergeMultipleAscending<T, St: Stream<Item = T>> {
        streams: Vec<Fuse<St>>,
        peeks: Vec<Option<T>>,
    }
}

impl<T: Ord, St: Stream<Item = T> + Unpin> Stream for MergeMultipleAscending<T, St> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let this = self.project();
        let mut vals = Vec::with_capacity(this.streams.len());
        vals.resize_with(this.streams.len(), || None);
        for (i, peek) in this.peeks.iter_mut().enumerate() {
            match peek.take() {
                Some(val) => vals[i] = Some(val),
                None => match this.streams[i].poll_next_unpin(cx) {
                    Ready(Some(val)) => vals[i] = Some(val),
                    Ready(None) => vals[i] = None,
                    Pending => {
                        // Clippy suggests
                        // for (j, <item>) in vals.iter_mut().enumerate().take(i) {
                        #[allow(clippy::needless_range_loop)]
                        for j in 0..i {
                            this.peeks[j] = vals[j].take()
                        }
                        return Pending;
                    }
                },
            }
        }
        let mut min_ix = None;
        let mut min_val = None;
        for (i, val) in vals.iter_mut().enumerate() {
            let val = val.take();
            if min_val.is_none() || val < min_val {
                if let Some(j) = min_ix {
                    this.peeks[j] = min_val;
                }
                min_val = val;
                min_ix = Some(i);
            } else {
                this.peeks[i] = val;
            }
        }
        Ready(min_val)
    }
}

impl<T: Ord, St: Stream<Item = T> + Unpin> FusedStream for MergeMultipleAscending<T, St> {
    fn is_terminated(&self) -> bool {
        self.peeks.iter().all(|peek| peek.is_none())
            && self.streams.iter().all(|stream| stream.is_terminated())
    }
}
