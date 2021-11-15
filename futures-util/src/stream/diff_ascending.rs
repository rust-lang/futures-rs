use std::cmp::Ordering;
use std::pin::Pin;
use std::task::{Context, Poll};
use Poll::*;

use crate::stream::{Fuse, StreamExt};
use futures_core::stream::{FusedStream, Stream};
use pin_project_lite::pin_project;

/// Diff two sorted streams in constant space. `diff_ascending(x, y)` returns a
/// stream containing all elements in `x` not present in `y`. This operation is
/// not commutative. The precondition (input sterams are ascending) is not
/// checked.
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter(0..10);
/// let s2 = stream::iter(0..5);
/// let collected: Vec<i32> = stream::diff_ascending(s1, s2).collect().await;
/// assert_eq!(collected, vec![5,6,7,8,9]);
/// # });
/// ```
pub fn diff_ascending<T: Ord, St1: Stream<Item = T>, St2: Stream<Item = T>>(
    st1: St1,
    st2: St2,
) -> DiffAscending<T, St1, St2> {
    DiffAscending {
        left: st1.fuse(),
        right: st2.fuse(),
        left_peek: None,
        right_peek: None,
        right_terminated: false,
    }
}

pin_project! {
    /// Struct for the [`diff_ascending`] method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct DiffAscending<T, St1: Stream<Item = T>, St2: Stream<Item = T>> {
        #[pin]
        left: Fuse<St1>,
        #[pin]
        right: Fuse<St2>,
        left_peek: Option<T>,
        right_peek: Option<T>,
        right_terminated: bool,
    }
}

impl<T: Ord, St1: Stream<Item = T>, St2: Stream<Item = T>> Stream for DiffAscending<T, St1, St2> {
    type Item = T;

    // For reference
    // diff :: Ord a => [a] -> [a] -> [a]
    // diff (x : xs) (y : ys)
    //   | x == y = diff xs ys
    //   | x < y = x : diff xs (y : ys)
    //   | x > y = diff (x : xs) ys
    // diff xs [] = xs
    // diff [] _ = []

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        if *this.right_terminated {
            return match this.left_peek.take() {
                Some(l) => Ready(Some(l)),
                None => this.left.poll_next(cx),
            };
        }
        loop {
            let l = match this.left_peek.take() {
                Some(l) => l,
                None => match this.left.as_mut().poll_next(cx) {
                    Ready(Some(x)) => x,
                    Ready(None) => return Ready(None),
                    Pending => return Pending,
                },
            };
            let r = match this.right_peek.take() {
                Some(r) => r,
                None => match this.right.as_mut().poll_next(cx) {
                    Ready(Some(x)) => x,
                    Ready(None) => {
                        *this.right_terminated = true;
                        return Ready(Some(l));
                    }
                    Pending => {
                        *this.left_peek = Some(l);
                        return Pending;
                    }
                },
            };
            match l.cmp(&r) {
                Ordering::Less => {
                    *this.right_peek = Some(r);
                    return Ready(Some(l));
                }
                Ordering::Equal => {}
                Ordering::Greater => *this.left_peek = Some(l),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (l_low, l_high) = self.left.size_hint();
        let (r_low, _) = self.right.size_hint();
        (l_low - r_low, l_high)
    }
}

impl<T: Ord, St1: Stream<Item = T>, St2: Stream<Item = T>> FusedStream
    for DiffAscending<T, St1, St2>
{
    fn is_terminated(&self) -> bool {
        self.left_peek.is_none()
            && self.right_peek.is_none()
            && self.left.is_terminated()
            && self.right.is_terminated()
    }
}
