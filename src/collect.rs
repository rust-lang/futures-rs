use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use lock::Lock;
use {Future, Callback, PollResult, IntoFuture};
use util;

/// A future which takes a list of futures and resolves with a vector of the
/// completed values.
///
/// This future is created with the `collect` method.
pub struct Collect<I>
    where I: IntoIterator + Send + 'static,
          I::Item: IntoFuture,
          I::IntoIter: Send + 'static,
{
    state: State<I>,
}

enum State<I>
    where I: IntoIterator + Send + 'static,
          I::Item: IntoFuture,
          I::IntoIter: Send + 'static,
{
    Local {
        cur: Option<<I::Item as IntoFuture>::Future>,
        remaining: I::IntoIter,
        result: Option<Vec<<I::Item as IntoFuture>::Item>>,
    },
    Scheduled(Arc<Scheduled<I>>),
    Done,
}

struct Scheduled<I>
    where I: IntoIterator + Send + 'static,
          I::Item: IntoFuture,
          I::IntoIter: Send + 'static,
{
    state: AtomicUsize,
    slot: Lock<Option<<I::Item as IntoFuture>::Future>>,
}

const CANCELED: usize = !0;

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
        state: State::Local {
            cur: i.next().map(IntoFuture::into_future),
            remaining: i,
            result: Some(Vec::new()),
        },
    }
}

impl<I> Future for Collect<I>
    where I: IntoIterator + Send + 'static,
          I::IntoIter: Send + 'static,
          I::Item: IntoFuture,
{
    type Item = Vec<<I::Item as IntoFuture>::Item>;
    type Error = <I::Item as IntoFuture>::Error;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
    {
        let state = match mem::replace(&mut self.state, State::Done) {
            State::Local { cur, remaining, result } => {
                (cur, remaining, result)
            }
            State::Scheduled(s) => {
                self.state = State::Scheduled(s);
                return g(Err(util::reused()))
            }
            State::Done => return g(Err(util::reused())),
        };
        let (cur, remaining, result) = state;
        let result = match result {
            Some(result) => result,
            None => return g(Err(util::reused())),
        };
        let cur = match cur {
            Some(cur) => cur,
            None => return g(Ok(result)),
        };
        let state = Arc::new(Scheduled {
            state: AtomicUsize::new(result.len()),
            slot: Lock::new(None),
        });
        Scheduled::run(&state, cur, remaining, result, g);

        self.state = State::Scheduled(state);
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

impl<I> Scheduled<I>
    where I: IntoIterator + Send + 'static,
          I::Item: IntoFuture,
          I::IntoIter: Send + 'static
{
    fn run<G>(state: &Arc<Scheduled<I>>,
              mut future: <I::Item as IntoFuture>::Future,
              remaining: I::IntoIter,
              result: Vec<<I::Item as IntoFuture>::Item>,
              g: G)
        where G: FnOnce(PollResult<Vec<<I::Item as IntoFuture>::Item>,
                                   <I::Item as IntoFuture>::Error>) + Send + 'static
    {
        let state2 = state.clone();
        let nth = result.len() + 1;
        future.schedule(move |res| {
            Scheduled::finish(&state2, res, remaining, result, g)
        });

        match state.state.compare_and_swap(nth - 1, nth, Ordering::SeqCst) {
            // Someone canceled in this window, we shouldn't continue so we
            // cancel the future.
            CANCELED => drop(future),

            // We have successfully moved forward. Store our future for someone
            // else to cancel. Lock acquisition can fail for two reasons:
            //
            // 1. Our future has finished, and someone else moved the world
            //    forward, and we're racing with them.
            // 2. Someone is canceling this future, and they're trying to see if
            //    a future is listed.
            //
            // We treat both cases by canceling our own future (primarily for
            // case 2). Canceling in case 1 won't have any affect as the future
            // is already finished.
            //
            // After we store our future, we check the state one last time to
            // ensure that if a cancellation came in while we were storing the
            // future we actually cancel the future.
            n if n <= nth - 1 => {
                match state.slot.try_lock() {
                    Some(mut slot) => *slot = Some(future),
                    None => return drop(future),
                }

                if state.state.load(Ordering::SeqCst) == CANCELED {
                    let f = state.slot.try_lock().and_then(|mut f| f.take());
                    drop(f);
                }
            }

            // Ok, looks like the world has moved on beyond us and we're no
            // longer needed (our fiture finished)
            _ => {}
        }
    }

    fn finish<G>(state: &Arc<Scheduled<I>>,
                 res: PollResult<<I::Item as IntoFuture>::Item,
                                 <I::Item as IntoFuture>::Error>,
                 mut remaining: I::IntoIter,
                 mut result: Vec<<I::Item as IntoFuture>::Item>,
                 g: G)
        where G: FnOnce(PollResult<Vec<<I::Item as IntoFuture>::Item>,
                                   <I::Item as IntoFuture>::Error>) + Send + 'static
    {
        match res {
            Ok(item) => result.push(item),
            Err(e) => return g(Err(e)),
        }
        match remaining.next() {
            Some(f) => Scheduled::run(state, f.into_future(), remaining, result, g),
            None => return g(Ok(result)),
        }
    }

    fn cancel(&self) {
        // Store CANCELED to indicate that we're done for good. Any future calls
        // to `run` above will see this and cancel futures, so we just need to
        // handle the case that there's a future stored already.
        //
        // We acquire the lock, and if a future is in there we cancel it. Note
        // that acquiring the lock can fail if someone else is already storing a
        // new future in it. We're guaranteed, however, that when they unlock
        // the lock they'll check the state again and see CANCELED, then
        // cancelling their future.
        self.state.store(CANCELED, Ordering::SeqCst);
        let f = self.slot.try_lock().and_then(|mut f| f.take());
        drop(f);
    }
}

impl<I> Drop for Collect<I>
    where I: IntoIterator + Send + 'static,
          I::Item: IntoFuture,
          I::IntoIter: Send + 'static,
{
    fn drop(&mut self) {
        if let State::Scheduled(ref s) = self.state {
            s.cancel();
        }
    }
}
