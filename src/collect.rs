use std::mem;
use std::sync::Arc;

use {Future, PollResult, IntoFuture, Wake, Tokens};
use util::{self, Collapsed};

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
    polled_once: bool,
}

/// Creates a future which represents a collection of the results of the futures
/// given.
///
/// The returned future will execute each underlying future one at a time,
/// collecting the results into a destincation `Vec<T>`. If any future returns
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
        polled_once: false,
    }
}

impl<I> Future for Collect<I>
    where I: IntoIterator + Send + 'static,
          I::IntoIter: Send + 'static,
          I::Item: IntoFuture,
{
    type Item = Vec<<I::Item as IntoFuture>::Item>;
    type Error = <I::Item as IntoFuture>::Error;


    fn poll(&mut self, tokens: &Tokens)
            -> Option<PollResult<Self::Item, Self::Error>> {
        let polled_once = mem::replace(&mut self.polled_once, true);
        loop {
            match self.cur {
                Some(ref mut cur) => {
                    match cur.poll(tokens) {
                        Some(Ok(e)) => self.result.push(e),

                        // If we hit an error, drop all our associated resources
                        // ASAP.
                        Some(Err(e)) => {
                            for f in self.remaining.by_ref() {
                                drop(f);
                            }
                            for f in self.result.drain(..) {
                                drop(f);
                            }
                            return Some(Err(e))
                        }

                        None => return None,
                    }
                }
                None if !polled_once => return Some(Ok(Vec::new())),
                None => return Some(Err(util::reused())),
            }

            self.cur = self.remaining.next()
                           .map(IntoFuture::into_future)
                           .map(Collapsed::Start);
            if self.cur.is_none() {
                return Some(Ok(mem::replace(&mut self.result, Vec::new())))
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if let Some(ref mut cur) = self.cur {
            cur.schedule(wake);
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
