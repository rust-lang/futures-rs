//! Definition of the `JoinAll` combinator, waiting for all of a list of futures
//! to finish.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::future::Future;
use core::iter::FromIterator;
use core::mem;
use core::pin::Pin;
use core::task::{Context, Poll};

#[derive(Debug)]
enum ElemState<F>
where
    F: Future,
{
    Pending(F),
    Done(Option<F::Output>),
}

impl<F> ElemState<F>
where
    F: Future,
{
    fn pending_pin_mut(self: Pin<&mut Self>) -> Option<Pin<&mut F>> {
        // Safety: Basic enum pin projection, no drop + optionally Unpin based
        // on the type of this variant
        match unsafe { self.get_unchecked_mut() } {
            ElemState::Pending(f) => Some(unsafe { Pin::new_unchecked(f) }),
            ElemState::Done(_) => None,
        }
    }

    fn take_done(self: Pin<&mut Self>) -> Option<F::Output> {
        // Safety: Going from pin to a variant we never pin-project
        match unsafe { self.get_unchecked_mut() } {
            ElemState::Pending(_) => None,
            ElemState::Done(output) => output.take(),
        }
    }
}

impl<F> Unpin for ElemState<F> where F: Future + Unpin {}

fn iter_pin_mut<T>(slice: Pin<&mut [T]>) -> impl Iterator<Item = Pin<&mut T>> {
    // Safety: `std` _could_ make this unsound if it were to decide Pin's
    // invariants aren't required to transmit through slices. Otherwise this has
    // the same safety as a normal field pin projection.
    unsafe { slice.get_unchecked_mut() }
        .iter_mut()
        .map(|t| unsafe { Pin::new_unchecked(t) })
}

/// Future for the [`join_all`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct JoinAll<F>
where
    F: Future,
{
    elems: Pin<Box<[ElemState<F>]>>,
}

impl<F> fmt::Debug for JoinAll<F>
where
    F: Future + fmt::Debug,
    F::Output: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinAll")
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
/// This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
///
/// # See Also
///
/// This is purposefully a very simple API for basic use-cases. In a lot of
/// cases you will want to use the more powerful
/// [`FuturesUnordered`][crate::stream::FuturesUnordered] APIs, some
/// examples of additional functionality that provides:
///
///  * Adding new futures to the set even after it has been started.
///
///  * Only polling the specific futures that have been woken. In cases where
///    you have a lot of futures this will result in much more efficient polling.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future::join_all;
///
/// async fn foo(i: u32) -> u32 { i }
///
/// let futures = vec![foo(1), foo(2), foo(3)];
///
/// assert_eq!(join_all(futures).await, [1, 2, 3]);
/// # });
/// ```
pub fn join_all<I>(i: I) -> JoinAll<I::Item>
where
    I: IntoIterator,
    I::Item: Future,
{
    let elems: Box<[_]> = i.into_iter().map(ElemState::Pending).collect();
    JoinAll {
        elems: elems.into(),
    }
}

impl<F> Future for JoinAll<F>
where
    F: Future,
{
    type Output = Vec<F::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut all_done = true;

        for mut elem in iter_pin_mut(self.elems.as_mut()) {
            if let Some(pending) = elem.as_mut().pending_pin_mut() {
                if let Poll::Ready(output) = pending.poll(cx) {
                    elem.set(ElemState::Done(Some(output)));
                } else {
                    all_done = false;
                }
            }
        }

        if all_done {
            let mut elems = mem::replace(&mut self.elems, Box::pin([]));
            let result = iter_pin_mut(elems.as_mut())
                .map(|e| e.take_done().unwrap())
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

/// Future for the [`join_all_or_first_error`] function.
pub struct JoinAllFirstError<F, T, E>
where
    F: Future<Output = Result<T, E>>,
{
    elems: Pin<Box<[ElemState<F>]>>,
}

impl<F, T, E> fmt::Debug for JoinAllFirstError<F, T, E>
where
    F: Future<Output = Result<T, E>> + fmt::Debug,
    F::Output: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinAllFirstError")
            .field("elems", &self.elems)
            .finish()
    }
}

/// Creates a future which represents a collection of the outputs of the futures
/// given.
///
/// The returned future will drive execution for all of its underlying futures,
/// collecting the results into a destination `Vec<T>` in the same order as they
/// were provided. Or, if any of them result in an Err<E> then that is returned.
///
/// This function is only available when the `std` or `alloc` feature of this
/// library is activated, and it is activated by default.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future::join_all_or_first_error;
///
/// async fn foo(i: u32) -> Result<u32, String> {
///     if i < 2 {
///         Ok(i)
///     } else {
///         Err("An Error".to_string())
///     }
/// }
///
/// let futures = vec![foo(1), foo(2), foo(3)];
///
/// assert_eq!(join_all_or_first_error(futures).await, Err("An Error".to_string()));
/// # });
/// ```
pub fn join_all_or_first_error<I, T, E>(i: I) -> JoinAllFirstError<I::Item, T, E>
where
    I: IntoIterator,
    I::Item: Future<Output = Result<T, E>>,
{
    let elems: Box<[_]> = i.into_iter().map(ElemState::Pending).collect();
    JoinAllFirstError {
        elems: elems.into(),
    }
}

impl<F, T, E> Future for JoinAllFirstError<F, T, E>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<Vec<T>, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut all_done = true;
        let mut error_returned = false;

        for mut elem in iter_pin_mut(self.elems.as_mut()) {
            if let Some(pending) = elem.as_mut().pending_pin_mut() {
                if let Poll::Ready(output) = pending.poll(cx) {
                    error_returned = output.is_err();
                    elem.set(ElemState::Done(Some(output)));

                    if error_returned {
                        break;
                    }
                } else {
                    all_done = false;
                }
            }
        }

        if error_returned {
            let mut elems = mem::replace(&mut self.elems, Box::pin([]));
            let result = iter_pin_mut(elems.as_mut())
                .map(|e| e.take_done())
                .find(|e| if let Some(Err(_)) = e { true } else { false })
                .unwrap() // the find option
                .unwrap() // the take_down option
                .err() // getting just the error
                .unwrap();
            Poll::Ready(Err(result))
        } else if all_done {
            let mut elems = mem::replace(&mut self.elems, Box::pin([]));
            let result = iter_pin_mut(elems.as_mut())
                .filter_map(|e| e.take_done().unwrap().ok())
                .collect();
            Poll::Ready(Ok(result))
        } else {
            Poll::Pending
        }
    }
}

impl<F, T, E> FromIterator<F> for JoinAllFirstError<F, T, E>
where
    F: Future<Output = Result<T, E>>,
{
    fn from_iter<I: IntoIterator<Item = F>>(iter: I) -> Self {
        join_all_or_first_error(iter)
    }
}
