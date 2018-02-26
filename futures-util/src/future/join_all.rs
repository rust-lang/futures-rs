//! Definition of the `JoinAll` combinator, waiting for all of a list of futures
//! to finish.

use std::prelude::v1::*;

use std::fmt;
use std::mem;
use std::iter::FromIterator;

use futures_core::{Future, IntoFuture, Poll, Async};
use futures_core::task;

#[derive(Debug)]
enum ElemState<F> where F: Future {
    Pending(F),
    Done(F::Item),
}

/// A future which takes a list of futures and resolves with a vector of the
/// completed values.
///
/// This future is created with the `join_all` method.
#[must_use = "futures do nothing unless polled"]
pub struct JoinAll<F>
    where F: Future,
{
    elems: Vec<ElemState<F>>,
}

impl<F> fmt::Debug for JoinAll<F>
    where F: Future + fmt::Debug,
          F::Item: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("JoinAll")
            .field("elems", &self.elems)
            .finish()
    }
}

/// Creates a future which represents a collection of the results of the futures
/// given.
///
/// The returned future will drive execution for all of its underlying futures,
/// collecting the results into a destination `Vec<T>` in the same order as they
/// were provided. If any future returns an error then all other futures will be
/// canceled and an error will be returned immediately. If all futures complete
/// successfully, however, then the returned future will succeed with a `Vec` of
/// all the successful results.
///
/// # Examples
///
/// ```
/// # extern crate futures;
/// use futures::prelude::*;
/// use futures::future::{join_all, ok, err};
///
/// # fn main() {
/// #
/// let f = join_all(vec![
///     ok::<u32, u32>(1),
///     ok::<u32, u32>(2),
///     ok::<u32, u32>(3),
/// ]);
/// let f = f.map(|x| {
///     assert_eq!(x, [1, 2, 3]);
/// });
///
/// let f = join_all(vec![
///     Box::new(ok::<u32, u32>(1)),
///     Box::new(err::<u32, u32>(2)),
///     Box::new(ok::<u32, u32>(3)),
/// ]);
/// let f = f.then(|x| {
///     assert_eq!(x, Err(2));
///     x
/// });
/// # }
/// ```
pub fn join_all<I>(i: I) -> JoinAll<<I::Item as IntoFuture>::Future>
    where I: IntoIterator,
          I::Item: IntoFuture,
{
    let elems = i.into_iter().map(|f| {
        ElemState::Pending(f.into_future())
    }).collect();
    JoinAll { elems }
}

impl<F> Future for JoinAll<F>
    where F: Future,
{
    type Item = Vec<F::Item>;
    type Error = F::Error;


    fn poll(&mut self, cx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        let mut all_done = true;

        for idx in 0 .. self.elems.len() {
            let done_val = match self.elems[idx] {
                ElemState::Pending(ref mut t) => {
                    match t.poll(cx) {
                        Ok(Async::Ready(v)) => Ok(v),
                        Ok(Async::Pending) => {
                            all_done = false;
                            continue
                        }
                        Err(e) => Err(e),
                    }
                }
                ElemState::Done(ref mut _v) => continue,
            };

            match done_val {
                Ok(v) => self.elems[idx] = ElemState::Done(v),
                Err(e) => {
                    // On completion drop all our associated resources
                    // ASAP.
                    self.elems = Vec::new();
                    return Err(e)
                }
            }
        }

        if all_done {
            let elems = mem::replace(&mut self.elems, Vec::new());
            let result = elems.into_iter().map(|e| {
                match e {
                    ElemState::Done(t) => t,
                    _ => unreachable!(),
                }
            }).collect();
            Ok(Async::Ready(result))
        } else {
            Ok(Async::Pending)
        }
    }
}

impl<F: IntoFuture> FromIterator<F> for JoinAll<F::Future> {
    fn from_iter<T: IntoIterator<Item=F>>(iter: T) -> Self {
        join_all(iter)
    }
}

#[test]
fn join_all_from_iter() {
    let f_ok = |a| {
        let r: Result<i32, ()> = Ok(a);
        r.into_future()
    };

    pub fn assert_done<T, F>(_f: F, _result: Result<T::Item, T::Error>)
        where T: Future,
              T::Item: Eq + fmt::Debug,
              T::Error: Eq + fmt::Debug,
              F: FnOnce() -> T,
    {
        //FIXME: write a real test somehow somewhere
    }

    assert_done(|| vec![f_ok(1), f_ok(2)].into_iter().collect::<JoinAll<_>>(), Ok(vec![1, 2]))
}
