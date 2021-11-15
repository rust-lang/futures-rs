use std::pin::Pin;
use std::task::{Context, Poll};
use Poll::*;

use crate::stream::{Fuse, StreamExt};
use futures_core::stream::{FusedStream, Stream};
use pin_project_lite::pin_project;

/// Merge two ordered streams in constant space. The precondition (input streams
/// are ascending) is not checked.
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
///
/// let evens = stream::iter((0..5).map(|x| x * 2));
/// let odds = stream::iter((0..5).map(|x| x * 2 + 1));
/// let collected: Vec<i32> = merge_ascending(evens, odds).collect().await;
/// assert_eq!(collected, vec![0,1,2,3,4,5,6,7,8,9]);
/// # });
/// ```
pub fn merge_ascending<T: Ord, St1: Stream<Item = T>, St2: Stream<Item = T>>(
    st1: St1,
    st2: St2,
) -> MergeAscending<T, St1, St2> {
    MergeAscending { left: st1.fuse(), right: st2.fuse(), left_peek: None, right_peek: None }
}

pin_project! {
    /// Struct for the `merge_ascending` method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct MergeAscending<T, St1: Stream<Item = T>, St2: Stream<Item = T>> {
        #[pin]
        left: Fuse<St1>,
        #[pin]
        right: Fuse<St2>,
        left_peek: Option<T>,
        right_peek: Option<T>,
    }
}

impl<T: Ord, St1: Stream<Item = T>, St2: Stream<Item = T>> Stream for MergeAscending<T, St1, St2> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let mut this = self.project();
        let l = match this.left_peek.take() {
            Some(l) => Some(l),
            None => match this.left.as_mut().poll_next(cx) {
                Ready(Some(l)) => Some(l),
                Ready(None) => None,
                Pending => return Pending,
            },
        };
        let r = match this.right_peek.take() {
            Some(r) => Some(r),
            None => match this.right.as_mut().poll_next(cx) {
                Ready(Some(r)) => Some(r),
                Ready(None) => None,
                Pending => {
                    *this.left_peek = l;
                    return Pending;
                }
            },
        };
        match (l, r) {
            (Some(l), Some(r)) if l <= r => {
                *this.right_peek = Some(r);
                Ready(Some(l))
            }
            (Some(l), Some(r)) => {
                *this.left_peek = Some(l);
                Ready(Some(r))
            }
            (Some(l), None) => Ready(Some(l)),
            (None, Some(r)) => Ready(Some(r)),
            (None, None) => Ready(None),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (l_low, l_high) = self.left.size_hint();
        let (r_low, r_high) = self.right.size_hint();
        let high = match (l_high, r_high) {
            (Some(l), Some(r)) => Some(l + r),
            _ => None,
        };
        (l_low + r_low, high)
    }
}

impl<T: Ord, St1: Stream<Item = T>, St2: Stream<Item = T>> FusedStream
    for MergeAscending<T, St1, St2>
{
    fn is_terminated(&self) -> bool {
        self.left_peek.is_none()
            && self.right_peek.is_none()
            && self.left.is_terminated()
            && self.right.is_terminated()
    }
}
