use std::mem;
use std::prelude::v1::*;

use {Future, IntoFuture, Poll, Async};

/// Future for the `fold_unordered` combinator, folds a list of
/// futures into one Future
///
/// This is created by this `fold_unordered` function.
#[must_use = "futures do nothing unless polled"]
pub struct FoldUnordered<T, F, R>
    where F: Fn(R, T::Item) -> R,
          T: Future
{
    fold: F,
    initial: Option<R>,
    remaining: Vec<Option<T>>,
}

/// Creates a new future which will fold a list of futures into one future
///
/// The function tries to execute each future in parallel, so the order
/// of items passed to the fold function may be different as the order of
/// futures in the initial list
pub fn fold_unordered<I, R, F>(i: I,
                               initial: R,
                               fold: F)
                               -> FoldUnordered<<I::Item as IntoFuture>::Future, F, R>
    where I: IntoIterator,
          I::Item: IntoFuture,
          F: Fn(R, <I::Item as IntoFuture>::Item) -> R
{
    FoldUnordered {
        initial: Some(initial),
        fold: fold,
        remaining: i.into_iter()
            .map(IntoFuture::into_future)
            .map(Some)
            .collect(),
    }
}

impl<T, R, F> Future for FoldUnordered<T, F, R>
    where F: Fn(R, T::Item) -> R,
          T: Future
{
    type Item = R;
    type Error = T::Error;


    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut ready = true;
        for i in self.remaining.iter_mut() {
            let mut replace = false;
            if let &mut Some(ref mut f) = i {
                match f.poll() {
                    Err(e) => return Err(e),
                    Ok(Async::NotReady) => {
                        ready = false;
                    }
                    Ok(Async::Ready(e)) => {
                        let ref fold = self.fold;
                        let item = mem::replace(&mut self.initial, None);
                        self.initial = Some(fold(item.unwrap(), e));
                        replace = true;
                    }
                }
            }
            if replace {
                mem::replace(i, None);
            }
        }
        if ready {
            let r = mem::replace(&mut self.initial, None);
            Ok(Async::Ready(r.unwrap()))
        } else {
            Ok(Async::NotReady)
        }
    }
}


/// Future for the `fold` combinator, folds a list of
/// futures into one Future
///
/// This is created by this `fold` function.
#[must_use = "futures do nothing unless polled"]
pub struct Fold<T, F, R>
    where F: Fn(R, T::Item) -> R,
          T: Future
{
    fold: F,
    initial: Option<R>,
    remaining: Vec<T>,
    pos: usize,
}

/// Creates a new future which will fold a list of futures into one future
///
/// This function does **not** attempt to execute each future in parallel,
/// so the order the order of items passed to the fold function is equal to
/// the initial order of futures in the list. See `fold_unordered` for an
/// implementation, that executes futures in parallel.
pub fn fold<I, R, F>(i: I,
                     initial: R,
                     fold: F)
                     -> Fold<<I::Item as IntoFuture>::Future, F, R>
    where I: IntoIterator,
          I::Item: IntoFuture,
          F: Fn(R, <I::Item as IntoFuture>::Item) -> R
{
    Fold {
        initial: Some(initial),
        fold: fold,
        remaining: i.into_iter()
            .map(IntoFuture::into_future)
            .collect(),
        pos: 0,
    }
}

impl<T, R, F> Future for Fold<T, F, R>
    where F: Fn(R, T::Item) -> R,
          T: Future
{
    type Item = R;
    type Error = T::Error;


    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        for f in self.remaining.iter_mut().skip(self.pos) {
            match f.poll() {
                Err(e) => return Err(e),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Ok(Async::Ready(e)) => {
                    let ref fold = self.fold;
                    let item = mem::replace(&mut self.initial, None);
                    self.initial = Some(fold(item.unwrap(), e));
                }
            }
        }
        let r = mem::replace(&mut self.initial, None);
        Ok(Async::Ready(r.unwrap()))
    }
}
