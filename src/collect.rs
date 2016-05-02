use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cell::AtomicCell;
use {Future, Callback, PollResult, PollError, IntoFuture};
use util;

pub struct Collect<I> where I: IntoIterator, I::Item: IntoFuture {
    state: State<I>,
}

enum State<I> where I: IntoIterator, I::Item: IntoFuture {
    Local {
        cur: Option<<I::Item as IntoFuture>::Future>,
        remaining: I::IntoIter,
        result: Option<Vec<<I::Item as IntoFuture>::Item>>,
    },
    Scheduled(Arc<Scheduled<I>>),
    Canceled,
    Done,
}

struct Scheduled<I> where I: IntoIterator, I::Item: IntoFuture {
    state: AtomicUsize,
    slot: AtomicCell<Option<<I::Item as IntoFuture>::Future>>,
}

const CANCELED: usize = !0;

pub fn collect<I>(i: I) -> Collect<I>
    where I: IntoIterator,
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

    fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
        let (cur, remaining, result) = match self.state {
            State::Local { ref mut cur, ref mut remaining, ref mut result } => {
                (cur, remaining, result)
            }
            State::Canceled => return Some(Err(PollError::Canceled)),
            State::Scheduled(..) |
            State::Done => return Some(Err(util::reused())),
        };
        loop {
            match cur.as_mut().map(Future::poll) {
                Some(Some(Ok(i))) => {
                    result.as_mut().unwrap().push(i);
                    *cur = remaining.next().map(IntoFuture::into_future);
                }
                Some(Some(Err(e))) => {
                    for item in remaining {
                        item.into_future().cancel();
                    }
                    return Some(Err(e))
                }
                Some(None) => return None,

                None => {
                    match result.take() {
                        Some(vec) => return Some(Ok(vec)),
                        None => return Some(Err(util::reused())),
                    }
                }
            }
        }
    }

    // fn await(&mut self) -> FutureResult<Self::Item, Self::Error> {
    //     if self.canceled {
    //         return Err(FutureError::Canceled)
    //     }
    //     let remaining = try!(util::opt2poll(self.remaining.as_mut()));
    //     let mut err = None;
    //     for item in self.cur.take().into_iter().chain(remaining) {
    //         if err.is_some() {
    //             item.cancel();
    //         } else {
    //             match item.await() {
    //                 Ok(item) => self.result.push(item),
    //                 Err(e) => err = Some(e),
    //             }
    //         }
    //     }
    //     match err {
    //         Some(err) => Err(err),
    //         None => Ok(mem::replace(&mut self.result, Vec::new()))
    //     }
    // }

    fn cancel(&mut self) {
        match self.state {
            State::Local { ref mut cur, ref mut remaining, .. } => {
                if let Some(mut cur) = cur.take() {
                    cur.cancel();
                }
                for future in remaining {
                    future.into_future().cancel();
                }
            }
            State::Scheduled(ref s) => s.cancel(),
            State::Canceled | State::Done => return,
        }
        self.state = State::Canceled;
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
    {
        let state = match mem::replace(&mut self.state, State::Done) {
            State::Local { cur, remaining, result } => {
                (cur, remaining, result)
            }
            State::Canceled => return g(Err(PollError::Canceled)),
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
            slot: AtomicCell::new(None),
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
            CANCELED => future.cancel(),

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
                match state.slot.borrow() {
                    Some(mut slot) => *slot = Some(future),
                    None => return future.cancel(),
                }

                if state.state.load(Ordering::SeqCst) == CANCELED {
                    let f = state.slot.borrow().and_then(|mut f| f.take());
                    if let Some(mut f) = f {
                        f.cancel();
                    }
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
            Err(e) => {
                for f in remaining {
                    f.into_future().cancel();
                }
                return g(Err(e))
            }
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
        let f = self.slot.borrow().and_then(|mut f| f.take());
        if let Some(mut f) = f {
            f.cancel();
        }
    }
}
