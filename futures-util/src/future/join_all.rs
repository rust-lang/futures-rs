//! Definition of the `JoinAll` combinator, waiting for all of a list of futures
//! to finish.

use std::fmt;
use std::future::Future;
use std::iter::FromIterator;
use std::mem;
use std::pin::Pin;
use std::prelude::v1::*;
use std::task::Poll;

#[derive(Debug)]
enum ElemState<F>
where
    F: Future,
{
    Pending(F),
    Done(F::Output),
}

/// A future which takes a list of futures and resolves with a vector of the
/// completed values.
///
/// This future is created with the `join_all` method.
#[must_use = "futures do nothing unless polled"]
pub struct JoinAll<F>
where
    F: Future,
{
    elems: Vec<ElemState<F>>,
}

impl<F> fmt::Debug for JoinAll<F>
where
    F: Future + fmt::Debug,
    F::Output: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("JoinAll")
            .field("elems", &self.elems)
            .finish()
    }
}

/// Creates a future which represents a collection of the outputs of the futures
/// given.
///
/// The returned future will drive execution for all of its underlying futures,
/// collecting the results into a destination `Vec<T>` in the same order as they
/// were provided.
///
/// # Examples
///
/// ```
/// use futures_util::future::{FutureExt, join_all, ready};
///
/// let f = join_all(vec![
///     ready::<u32>(1),
///     ready::<u32>(2),
///     ready::<u32>(3),
/// ]);
/// let f = f.map(|x| {
///     assert_eq!(x, [1, 2, 3]);
/// });
/// ```
pub fn join_all<I>(i: I) -> JoinAll<I::Item>
where
    I: IntoIterator,
    I::Item: Future,
{
    let elems = i.into_iter().map(ElemState::Pending).collect();
    JoinAll { elems }
}

impl<F> Future for JoinAll<F>
where
    F: Future + Unpin,
    F::Output: Unpin,
{
    type Output = Vec<F::Output>;

    fn poll(
        mut self: Pin<&mut Self>,
        lw: &::std::task::LocalWaker,
    ) -> Poll<Self::Output> {
        let mut all_done = true;

        for elem in self.as_mut().elems.iter_mut() {
            match elem {
                ElemState::Pending(ref mut t) => match Pin::new(t).poll(lw) {
                    Poll::Ready(v) => *elem = ElemState::Done(v),
                    Poll::Pending => {
                        all_done = false;
                        continue;
                    }
                },
                ElemState::Done(ref mut _v) => (),
            };
        }

        if all_done {
            let elems = mem::replace(&mut self.elems, Vec::new());
            let result = elems
                .into_iter()
                .map(|e| match e {
                    ElemState::Done(t) => t,
                    _ => unreachable!(),
                })
                .collect();
            Poll::Ready(result)
        } else {
            Poll::Pending
        }
    }
}

impl<F: Future> FromIterator<F> for JoinAll<F> {
    fn from_iter<T: IntoIterator<Item = F>>(iter: T) -> Self {
        join_all(iter)
    }
}
