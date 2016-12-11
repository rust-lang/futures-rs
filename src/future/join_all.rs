//! Definition of the JoinAll combinator, waiting for all of a list of futures
//! to finish.

use std::prelude::v1::*;

use std::mem;

use {Future, IntoFuture, Poll, Async};

enum ElemState<T> where T: Future {
    Pending(T),
    Done(T::Item),
}

/// A future which takes a list of futures and resolves with a vector of the
/// completed values.
///
/// This future is created with the `join_all` method.
#[must_use = "futures do nothing unless polled"]
pub struct JoinAll<Item>
    where Item: IntoFuture,
{
    elems: Vec<ElemState<<Item as IntoFuture>::Future>>,
}

/// Creates a future which represents a collection of the results of the futures
/// given.
///
/// The returned future will drive execution for all of its underlying futures,
/// collecting the results into a destination `Vec<T>`. If any future returns
/// an error then all other futures will be canceled and an error will be
/// returned immediately. If all futures complete successfully, however, then
/// the returned future will succeed with a `Vec` of all the successful results.
///
/// Note that this function does **not** attempt to execute each future in
/// parallel, they are all executed in sequence.
///
/// # Examples
///
/// ```
/// use futures::future::*;
///
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
///     ok::<u32, u32>(1).boxed(),
///     err::<u32, u32>(2).boxed(),
///     ok::<u32, u32>(3).boxed(),
/// ]);
/// let f = f.then(|x| {
///     assert_eq!(x, Err(2));
///     x
/// });
/// ```
pub fn join_all<I>(i: I) -> JoinAll<I::Item>
    where I: IntoIterator,
          I::Item: IntoFuture,
{
    let elems = i.into_iter().map(|f| {
        ElemState::Pending(f.into_future())
    }).collect();
    JoinAll { elems: elems }
}

impl<Item> Future for JoinAll<Item>
    where Item: IntoFuture,
{
    type Item = Vec<<Item as IntoFuture>::Item>;
    type Error = <Item as IntoFuture>::Error;


    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut all_done = true;

        for idx in 0 .. self.elems.len() {
            let done_val = match &mut self.elems[idx] {
                &mut ElemState::Pending(ref mut t) => {
                    match t.poll() {
                        Ok(Async::Ready(v)) => Ok(v),
                        Ok(Async::NotReady) => {
                            all_done = false;
                            continue
                        }
                        Err(e) => Err(e),
                    }
                }
                &mut ElemState::Done(ref mut _v) => continue,
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
            Ok(Async::NotReady)
        }
    }
}
