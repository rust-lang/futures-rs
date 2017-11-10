//! An unbounded set of futures.

use std::iter::FromIterator;

use {task, Stream, Future, Poll, Async};
use scheduler::{self, Scheduler};
use task_impl::{self, AtomicTask};

/// An unbounded set of futures.
///
/// This "combinator" also serves a special function in this library, providing
/// the ability to maintain a set of futures that and manage driving them all
/// to completion.
///
/// Futures are pushed into this set and their realized values are yielded as
/// they are ready. This structure is optimized to manage a large number of
/// futures. Futures managed by `FuturesUnordered` will only be polled when they
/// generate notifications. This reduces the required amount of work needed to
/// coordinate large numbers of futures.
///
/// When a `FuturesUnordered` is first created, it does not contain any futures.
/// Calling `poll` in this state will result in `Ok(Async::Ready(None))` to be
/// returned. Futures are submitted to the set using `push`; however, the
/// future will **not** be polled at this point. `FuturesUnordered` will only
/// poll managed futures when `FuturesUnordered::poll` is called. As such, it
/// is important to call `poll` after pushing new futures.
///
/// If `FuturesUnordered::poll` returns `Ok(Async::Ready(None))` this means that
/// the set is currently not managing any futures. A future may be submitted
/// to the set at a later time. At that point, a call to
/// `FuturesUnordered::poll` will either return the future's resolved value
/// **or** `Ok(Async::NotReady)` if the future has not yet completed.
///
/// Note that you can create a ready-made `FuturesUnordered` via the
/// `futures_unordered` function in the `stream` module, or you can start with an
/// empty set with the `FuturesUnordered::new` constructor.
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct FuturesUnordered<T> {
    inner: Scheduler<T, AtomicTask>,
}

impl<T> FuturesUnordered<T>
    where T: Future,
{
    /// Constructs a new, empty `FuturesUnordered`
    ///
    /// The returned `FuturesUnordered` does not contain any futures and, in this
    /// state, `FuturesUnordered::poll` will return `Ok(Async::Ready(None))`.
    pub fn new() -> FuturesUnordered<T> {
        let inner = Scheduler::new(AtomicTask::new());
        FuturesUnordered { inner: inner }
    }
}

impl<T> FuturesUnordered<T> {
    /// Returns the number of futures contained in the set.
    ///
    /// This represents the total number of in-flight futures.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the set contains no futures
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Push a future into the set.
    ///
    /// This function submits the given future to the set for managing. This
    /// function will not call `poll` on the submitted future. The caller must
    /// ensure that `FuturesUnordered::poll` is called in order to receive task
    /// notifications.
    pub fn push(&mut self, future: T) {
        self.inner.push(future)
    }

    /// Returns an iterator that allows modifying each future in the set.
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            inner: self.inner.iter_mut(),
        }
    }
}

impl<T> Stream for FuturesUnordered<T>
    where T: Future
{
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Option<T::Item>, T::Error> {
        use scheduler::Tick;

        // Ensure `parent` is correctly set.
        self.inner.get_wakeup().register();

        let res = self.inner.tick(|_, f, notify| {
            match task_impl::with_notify(notify, 0, || f.poll()) {
                Ok(Async::Ready(v)) => Async::Ready(Ok(v)),
                Ok(Async::NotReady) => Async::NotReady,
                Err(e) => Async::Ready(Err(e)),
            }
        });

        match res {
            Tick::Data(Ok(v)) => Ok(Async::Ready(Some(v))),
            Tick::Data(Err(e)) => Err(e),
            Tick::Empty => {
                if self.is_empty() {
                    Ok(Async::Ready(None))
                } else {
                    Ok(Async::NotReady)
                }
            }
            Tick::Inconsistent => {
                // At this point, it may be worth yielding the thread &
                // spinning a few times... but for now, just yield using the
                // task system.
                //
                // TODO: Don't do this here
                task::current().notify();
                return Ok(Async::NotReady);
            }
        }
    }
}

impl<F: Future> FromIterator<F> for FuturesUnordered<F> {
    fn from_iter<T>(iter: T) -> Self
        where T: IntoIterator<Item = F>
    {
        let mut new = FuturesUnordered::new();
        for future in iter.into_iter() {
            new.push(future);
        }
        new
    }
}

#[derive(Debug)]
/// Mutable iterator over all futures in the unordered set.
pub struct IterMut<'a, F: 'a> {
    inner: scheduler::IterMut<'a, F, AtomicTask>,
}

impl<'a, F> Iterator for IterMut<'a, F> {
    type Item = &'a mut F;

    fn next(&mut self) -> Option<&'a mut F> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, F> ExactSizeIterator for IterMut<'a, F> {}
