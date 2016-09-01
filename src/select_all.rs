use std::mem;
use std::prelude::v1::*;

use {Future, IntoFuture, Poll, Async};

/// Future for the `select_all` combinator, waiting for one of any of a list of
/// futures to complete.
///
/// This is created by this `select_all` function.
pub struct SelectAll<A> where A: Future {
    inner: Vec<SelectAllNext<A>>,
}

/// Future yielded as the result in a `SelectAll` future.
///
/// This sentinel future represents the completion of the remaining futures in a
/// list of futures.
pub struct SelectAllNext<A> where A: Future {
    inner: A,
}

/// Creates a new future which will select over a list of futures.
///
/// The returned future will wait for any future within `list` to be ready. Upon
/// completion or failure the item resolved will be returned, along with the
/// index of the future that was ready and the list of all the remaining
/// futures.
///
/// # Panics
///
/// This function will panic if the iterator specified contains no items.
pub fn select_all<I>(iter: I) -> SelectAll<<I::Item as IntoFuture>::Future>
    where I: IntoIterator,
          I::Item: IntoFuture,
{
    let ret = SelectAll {
        inner: iter.into_iter()
                   .map(|a| a.into_future())
                   .map(|a| SelectAllNext { inner: a })
                   .collect(),
    };
    assert!(ret.inner.len() > 0);
    ret
}

impl<A> Future for SelectAll<A>
    where A: Future,
{
    type Item = (A::Item, usize, Vec<SelectAllNext<A>>);
    type Error = (A::Error, usize, Vec<SelectAllNext<A>>);

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let item = self.inner.iter_mut().enumerate().filter_map(|(i, f)| {
            match f.poll() {
                Ok(Async::NotReady) => None,
                Ok(Async::Ready(e)) => Some((i, Ok(e))),
                Err(e) => Some((i, Err(e))),
            }
        }).next();
        match item {
            Some((idx, res)) => {
                self.inner.remove(idx);
                let rest = mem::replace(&mut self.inner, Vec::new());
                match res {
                    Ok(e) => Ok(Async::Ready((e, idx, rest))),
                    Err(e) => Err((e, idx, rest)),
                }
            }
            None => Ok(Async::NotReady),
        }
    }
}

impl<A> Future for SelectAllNext<A>
    where A: Future,
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.inner.poll()
    }
}
