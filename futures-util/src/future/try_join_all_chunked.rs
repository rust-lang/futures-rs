//! Definition of the `TryJoinAllChunked` combinator, waiting for all of a list of
//! futures to finish with either success or error whilst only running at most n at a time.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::future::Future;
use core::mem;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::{assert_future, TryFuture, TryMaybeDone};


fn iter_pin_mut<T>(slice: Pin<&mut [T]>) -> impl Iterator<Item = Pin<&mut T>> {
    // Safety: `std` _could_ make this unsound if it were to decide Pin's
    // invariants aren't required to transmit through slices. Otherwise this has
    // the same safety as a normal field pin projection.
    unsafe { slice.get_unchecked_mut() }.iter_mut().map(|t| unsafe { Pin::new_unchecked(t) })
}

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryJoinAllChunked<F>
where
    F: TryFuture,
{
    elems: Pin<Box<[TryMaybeDone<F>]>>,
    chunk_size: usize
}

impl<F> fmt::Debug for TryJoinAllChunked<F>
where
    F: TryFuture + fmt::Debug,
    F::Ok: fmt::Debug,
    F::Error: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryJoinAllChunked").field("elems", &self.elems).finish()
    }
}

/// Creates a future which represents either a collection of the results of the
/// futures given or an error.
///
/// The returned future will drive execution for all of its underlying futures,
/// driving at most n at a time, collecting the results into a destination `Vec<T>`
/// in the same order as they were provided.
///
/// If any future returns an error then all other futures will be canceled and
/// an error will be returned immediately. If all futures complete successfully,
/// however, then the returned future will succeed with a `Vec` of all the
/// successful results.
///
/// This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future::{self, try_join_all};
///
/// let futures = vec![
///     future::ok::<u32, u32>(1),
///     future::ok::<u32, u32>(2),
///     future::ok::<u32, u32>(3),
/// ];
///
/// assert_eq!(try_join_all_chunked(futures, 1).await, Ok(vec![1, 2, 3]));
///
/// let futures = vec![
///     future::ok::<u32, u32>(1),
///     future::err::<u32, u32>(2),
///     future::ok::<u32, u32>(3),
/// ];
///
/// assert_eq!(try_join_all_chunked(futures).await, Err(2));
/// # });
/// ```
pub fn try_join_all_chunked<I>(i: I, n: usize) -> TryJoinAllChunked<I::Item>
where
    I: IntoIterator,
    I::Item: TryFuture,
{
    let elems: Box<[_]> = i.into_iter().map(TryMaybeDone::Future).collect();
    assert_future::<Result<Vec<<I::Item as TryFuture>::Ok>, <I::Item as TryFuture>::Error>, _>(
        TryJoinAllChunked { elems: elems.into(), chunk_size:n },
    )
}

impl<F> Future for TryJoinAllChunked<F>
where
    F: TryFuture,
{
    type Output = Result<Vec<F::Ok>, F::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = FinalState::AllDone;
        let mut in_progress = 0;

        for elem in iter_pin_mut(self.elems.as_mut()) {
            if in_progress < self.chunk_size
            {
                match elem.try_poll(cx) {
                    Poll::Pending => {
                        in_progress += 1;
                        state = FinalState::Pending;
                    },
                    Poll::Ready(Ok(())) => {}
                    Poll::Ready(Err(e)) => {
                        state = FinalState::Error(e);
                        break;
                    }
                }
            }
            else {
                break;
            }
        }

        match state {
            FinalState::Pending => Poll::Pending,
            FinalState::AllDone => {
                let mut elems = mem::replace(&mut self.elems, Box::pin([]));
                let results =
                    iter_pin_mut(elems.as_mut()).map(|e| e.take_output().unwrap()).collect();
                Poll::Ready(Ok(results))
            }
            FinalState::Error(e) => {
                let _ = mem::replace(&mut self.elems, Box::pin([]));
                Poll::Ready(Err(e))
            }
        }
    }
}
