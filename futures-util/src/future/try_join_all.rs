//! Definition of the `TryJoinAll` combinator, waiting for all of a list of
//! futures to finish with either success or error.

use alloc::{boxed::Box, vec::Vec};
use core::{
    fmt,
    future::Future,
    iter::FromIterator,
    mem,
    pin::Pin,
    task::{Context, Poll},
};

#[cfg(target_has_atomic = "ptr")]
use pin_project_lite::pin_project;

use super::{IntoFuture, TryFuture, TryMaybeDone, assert_future, join_all};
use crate::TryFutureExt;
#[cfg(target_has_atomic = "ptr")]
use crate::stream::{FuturesUnordered, StreamExt};

#[cfg(target_has_atomic = "ptr")]
pin_project! {
    #[derive(Debug)]
    struct Indexed<F> {
        index: usize,
        #[pin]
        future: F,
    }
}

#[cfg(target_has_atomic = "ptr")]
impl<F: Future> Future for Indexed<F> {
    type Output = (usize, F::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let index = *this.index;
        this.future.poll(cx).map(|output| (index, output))
    }
}

enum FinalState<E = ()> {
    Pending,
    AllDone,
    Error(E),
}

/// Future for the [`try_join_all`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryJoinAll<F>
where
    F: TryFuture,
{
    kind: TryJoinAllKind<F>,
}

// Futures are pinned within their collections; completed outputs are not pinned.
impl<F: TryFuture> Unpin for TryJoinAll<F> {}

enum TryJoinAllKind<F>
where
    F: TryFuture,
{
    Small {
        elems: Pin<Box<[TryMaybeDone<IntoFuture<F>>]>>,
    },
    #[cfg(target_has_atomic = "ptr")]
    Big {
        futures: FuturesUnordered<Indexed<IntoFuture<F>>>,
        outputs: Vec<F::Ok>,
        indices: Vec<usize>,
    },
}

impl<F> fmt::Debug for TryJoinAll<F>
where
    F: TryFuture + fmt::Debug,
    F::Ok: fmt::Debug,
    F::Error: fmt::Debug,
    F::Output: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TryJoinAllKind::Small { ref elems } => {
                f.debug_struct("TryJoinAll").field("elems", elems).finish()
            }
            #[cfg(target_has_atomic = "ptr")]
            TryJoinAllKind::Big { ref futures, ref outputs, .. } => f
                .debug_struct("TryJoinAll")
                .field("futures", futures)
                .field("outputs", outputs)
                .finish(),
        }
    }
}

/// Creates a future which represents either a collection of the results of the
/// futures given or an error.
///
/// The returned future will drive execution for all of its underlying futures,
/// collecting the results into a destination `Vec<T>` in the same order as they
/// were provided.
///
/// If any future returns an error then all other futures will be canceled and
/// an error will be returned immediately. If all futures complete successfully,
/// however, then the returned future will succeed with a `Vec` of all the
/// successful results.
///
/// This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
///
/// # See Also
///
/// `try_join_all` will switch to the more powerful [`FuturesUnordered`] for performance
/// reasons if the number of futures is large. You may want to look into using it or
/// its counterpart [`FuturesOrdered`][crate::stream::FuturesOrdered] directly.
///
/// Some examples for additional functionality provided by these are:
///
///  * Adding new futures to the set even after it has been started.
///
///  * Only polling the specific futures that have been woken. In cases where
///    you have a lot of futures this will result in much more efficient polling.
///
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
/// assert_eq!(try_join_all(futures).await, Ok(vec![1, 2, 3]));
///
/// let futures = vec![
///     future::ok::<u32, u32>(1),
///     future::err::<u32, u32>(2),
///     future::ok::<u32, u32>(3),
/// ];
///
/// assert_eq!(try_join_all(futures).await, Err(2));
/// # });
/// ```
pub fn try_join_all<I>(iter: I) -> TryJoinAll<I::Item>
where
    I: IntoIterator,
    I::Item: TryFuture,
{
    let iter = iter.into_iter().map(TryFutureExt::into_future);

    #[cfg(not(target_has_atomic = "ptr"))]
    {
        let kind = TryJoinAllKind::Small {
            elems: iter.map(TryMaybeDone::Future).collect::<Box<[_]>>().into(),
        };

        assert_future::<Result<Vec<<I::Item as TryFuture>::Ok>, <I::Item as TryFuture>::Error>, _>(
            TryJoinAll { kind },
        )
    }

    #[cfg(target_has_atomic = "ptr")]
    {
        let kind = match iter.size_hint().1 {
            Some(max) if max <= join_all::SMALL => TryJoinAllKind::Small {
                elems: iter.map(TryMaybeDone::Future).collect::<Box<[_]>>().into(),
            },
            _ => {
                let futures: FuturesUnordered<_> =
                    iter.enumerate().map(|(index, future)| Indexed { index, future }).collect();
                TryJoinAllKind::Big { futures, outputs: Vec::new(), indices: Vec::new() }
            }
        };

        assert_future::<Result<Vec<<I::Item as TryFuture>::Ok>, <I::Item as TryFuture>::Error>, _>(
            TryJoinAll { kind },
        )
    }
}

impl<F> Future for TryJoinAll<F>
where
    F: TryFuture,
{
    type Output = Result<Vec<F::Ok>, F::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.kind {
            TryJoinAllKind::Small { elems } => {
                let mut state = FinalState::AllDone;

                for elem in join_all::iter_pin_mut(elems.as_mut()) {
                    match elem.try_poll(cx) {
                        Poll::Pending => state = FinalState::Pending,
                        Poll::Ready(Ok(())) => {}
                        Poll::Ready(Err(e)) => {
                            state = FinalState::Error(e);
                            break;
                        }
                    }
                }

                match state {
                    FinalState::Pending => Poll::Pending,
                    FinalState::AllDone => {
                        let mut elems = mem::replace(elems, Box::pin([]));
                        let results = join_all::iter_pin_mut(elems.as_mut())
                            .map(|e| e.take_output().unwrap())
                            .collect();
                        Poll::Ready(Ok(results))
                    }
                    FinalState::Error(e) => {
                        let _ = mem::replace(elems, Box::pin([]));
                        Poll::Ready(Err(e))
                    }
                }
            }
            #[cfg(target_has_atomic = "ptr")]
            TryJoinAllKind::Big { futures, outputs, indices } => {
                let result = loop {
                    match futures.poll_next_unpin(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Some((index, Ok(output)))) => {
                            indices.push(index);
                            outputs.push(output);
                        }
                        Poll::Ready(Some((_, Err(error)))) => break Err(error),
                        Poll::Ready(None) => {
                            for i in 0..indices.len() {
                                while indices[i] != i {
                                    // Each swap places one result at its original input index.
                                    let index = indices[i];
                                    indices.swap(i, index);
                                    outputs.swap(i, index);
                                }
                            }
                            break Ok(mem::take(outputs));
                        }
                    }
                };
                futures.clear();
                outputs.clear();
                indices.clear();
                Poll::Ready(result)
            }
        }
    }
}

impl<F> FromIterator<F> for TryJoinAll<F>
where
    F: TryFuture,
{
    fn from_iter<T: IntoIterator<Item = F>>(iter: T) -> Self {
        try_join_all(iter)
    }
}
