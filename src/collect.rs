use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cell::AtomicCell;
use {Future, Callback, PollResult, PollError};
use util;

pub struct Collect<I> where I: IntoIterator, I::Item: Future {
    state: State<I>,
}

enum State<I> where I: IntoIterator, I::Item: Future {
    Local {
        cur: Option<I::Item>,
        remaining: I::IntoIter,
        result: Vec<<I::Item as Future>::Item>,
    },
    Scheduled(Arc<Scheduled<I>>),
    Canceled,
}

struct Scheduled<I> where I: IntoIterator, I::Item: Future {
    state: AtomicUsize,
    slot: AtomicCell<Option<I::Item>>,
}

const EMPTY: usize = 0;
const FULL: usize = 1;
const CANCEL: usize = 2;

pub fn collect<I>(i: I) -> Collect<I>
    where I: IntoIterator,
          I::Item: Future,
          I::IntoIter: Send + 'static,
{
    let mut i = i.into_iter();
    Collect {
        state: State::Local {
            cur: i.next(),
            remaining: i,
            result: Vec::new(),
        },
    }
}

impl<I> Future for Collect<I>
    where I: IntoIterator + Send + 'static,
          I::IntoIter: Send + 'static,
          I::Item: Future,
{
    type Item = Vec<<I::Item as Future>::Item>;
    type Error = <I::Item as Future>::Error;

    fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
        let (cur, remaining, result) = match self.state {
            State::Local { ref mut cur, ref mut remaining, ref mut result } => {
                (cur, remaining, result)
            }
            State::Canceled => return Some(Err(PollError::Canceled)),
            State::Scheduled(..) => return Some(Err(util::reused())),
        };
        loop {
            match cur.as_mut().map(Future::poll) {
                Some(Some(Ok(i))) => {
                    result.push(i);
                    *cur = remaining.next();
                }
                Some(Some(Err(e))) => {
                    for mut item in remaining {
                        item.cancel();
                    }
                    return Some(Err(e))
                }
                Some(None) => return None,

                // TODO: two poll() calls should probably panic the second
                None => return Some(Ok(mem::replace(result, Vec::new()))),
            }
        }
    }

    // fn await(&mut self) -> FutureResult<Self::Item, Self::Error> {
    //     if self.canceled {
    //         return Err(FutureError::Canceled)
    //     }
    //     let remaining = try!(util::opt2poll(self.remaining.as_mut()));
    //     let mut err = None;
    //     for mut item in self.cur.take().into_iter().chain(remaining) {
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
            State::Local { ref mut cur, ref mut remaining, ref mut result } => {
                if let Some(mut cur) = cur.take() {
                    cur.cancel();
                }
                for mut future in remaining {
                    future.cancel();
                }
            }
            State::Scheduled(ref s) => {
                loop {}
            }
            State::Canceled => return,
        }
        self.state = State::Canceled;
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
    {
        let state = match mem::replace(&mut self.state, State::Canceled) {
            State::Local { cur, remaining, result } => (cur, remaining, result),
            State::Canceled => return g(Err(PollError::Canceled)),
            State::Scheduled(s) => {
                self.state = State::Scheduled(s);
                return g(Err(util::reused()))
            }
        };
        let (cur, remaining, result) = state;
        let mut cur = match cur {
            Some(cur) => cur,
            None => return g(Ok(result)),
        };
        let state = Arc::new(Scheduled {
            state: AtomicUsize::new(EMPTY),
            slot: AtomicCell::new(None),
        });
        let state2 = state.clone();
        cur.schedule(move |res| {
            drop((remaining, result));
            // let item = match res {
            //     Ok(i) => i,
            //     Err(e) => {
            //         for mut future in remaining {
            //             future.cancel();
            //         }
            //         return g(Err(e))
            //     }
            // };
            // result.push(item);
            // Collect {
            //     cur: remaining.next(),
            //     remaining: Some(remaining),
            //     result: result,
            // }.schedule(g);
        });

        *state2.slot.borrow().expect("couldn't pick up slot") = Some(cur);

        self.state = State::Scheduled(state2);
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

impl<I> Scheduled<I> where I: IntoIterator, I::Item: Future {
}
