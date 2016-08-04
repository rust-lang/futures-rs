use std::mem;

use {Future, IntoFuture, Task, Poll};
use util::Collapsed;

/// A future which takes a list of futures and resolves with a vector of the
/// completed values.
///
/// This future is created with the `collect` method.
pub struct Collect<I>
    where I: IntoIterator + Send + 'static,
          I::Item: IntoFuture,
          I::IntoIter: Send + 'static,
{
    cur: Option<Collapsed<<I::Item as IntoFuture>::Future>>,
    remaining: I::IntoIter,
    result: Vec<<I::Item as IntoFuture>::Item>,
}

/// Creates a future which represents a collection of the results of the futures
/// given.
///
/// The returned future will execute each underlying future one at a time,
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
/// use futures::*;
///
/// let f = collect(vec![
///     finished::<u32, u32>(1),
///     finished::<u32, u32>(2),
///     finished::<u32, u32>(3),
/// ]);
/// let f = f.map(|x| {
///     assert_eq!(x, [1, 2, 3]);
/// });
///
/// let f = collect(vec![
///     finished::<u32, u32>(1).boxed(),
///     failed::<u32, u32>(2).boxed(),
///     finished::<u32, u32>(3).boxed(),
/// ]);
/// let f = f.then(|x| {
///     assert_eq!(x, Err(2));
///     x
/// });
/// ```
pub fn collect<I>(i: I) -> Collect<I>
    where I: IntoIterator + Send + 'static,
          I::Item: IntoFuture,
          I::IntoIter: Send + 'static,
{
    let mut i = i.into_iter();
    Collect {
        cur: i.next().map(IntoFuture::into_future).map(Collapsed::Start),
        remaining: i,
        result: Vec::new(),
    }
}

impl<I> Future for Collect<I>
    where I: IntoIterator + Send + 'static,
          I::IntoIter: Send + 'static,
          I::Item: IntoFuture,
{
    type Item = Vec<<I::Item as IntoFuture>::Item>;
    type Error = <I::Item as IntoFuture>::Error;


    fn poll(&mut self, task: &mut Task) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.cur {
                Some(ref mut cur) => {
                    match try_poll!(cur.poll(task)) {
                        Ok(e) => self.result.push(e),

                        // If we hit an error, drop all our associated resources
                        // ASAP.
                        Err(e) => {
                            for f in self.remaining.by_ref() {
                                drop(f);
                            }
                            for f in self.result.drain(..) {
                                drop(f);
                            }
                            return Poll::Err(e)
                        }
                    }
                }
                None => {
                    return Poll::Ok(mem::replace(&mut self.result, Vec::new()))
                }
            }

            self.cur = self.remaining.next()
                           .map(IntoFuture::into_future)
                           .map(Collapsed::Start);
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if let Some(ref mut cur) = self.cur {
            cur.schedule(task);
        }
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        if let Some(ref mut cur) = self.cur {
            cur.collapse();
        }
        None
    }

}
