//! Definition of the MaybeDone combinator

use core::mem::{self, PinMut};
use core::marker::Unpin;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};

/// `MaybeDone`, a future that may have completed.
///
/// This is created by the `maybe_done` function.
#[derive(Debug)]
pub enum MaybeDone<F: Future> {
    /// A not-yet-completed future
    Future(F),
    /// The output of the completed future
    Done(F::Output),
    /// The empty variant after the result of a `maybe_done` has been
    /// taken using the `take_output` method.
    Gone,
}

// safe because we never generate `PinMut<A::Output>`
impl<F: Future + Unpin> Unpin for MaybeDone<F> {}

/// Creates a new future from a closure.
pub fn maybe_done<F: Future>(f: F) -> MaybeDone<F> {
    MaybeDone::Future(f)
}

impl<F: Future> MaybeDone<F> {
    /// Attempt to take the output of a `MaybeDone` without driving it
    /// towards completion.
    pub fn take_output(self: PinMut<Self>) -> Option<F::Output> {
        unsafe {
            let this = PinMut::get_mut_unchecked(self);
            match this {
                MaybeDone::Done(_) => {},
                MaybeDone::Future(_) | MaybeDone::Gone => return None,
            };
            if let MaybeDone::Done(output) = mem::replace(this, MaybeDone::Gone) {
                Some(output)
            } else {
                unreachable!()
            }
        }
    }
}

impl<A: Future> Future for MaybeDone<A> {
    type Output = ();

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Self::Output> {
        let res = unsafe {
            match PinMut::get_mut_unchecked(self.reborrow()) {
                MaybeDone::Future(a) => {
                    if let Poll::Ready(res) = PinMut::new_unchecked(a).poll(cx) {
                        res
                    } else {
                        return Poll::Pending
                    }
                }
                MaybeDone::Done(_) => return Poll::Ready(()),
                MaybeDone::Gone => panic!("MaybeDone polled after value taken"),
            }
        };
        PinMut::set(self, MaybeDone::Done(res));
        Poll::Ready(())
    }
}
