use {Future, Poll, Async, task};

use core::mem;

enum State<A>
    where A: Future,
{
    NotReady(A),
    Ready(Poll<A::Item, A::Error>),
}

/// Future for the `then_yield` combiner, returning not ready once after
/// completing the inner future.
///
/// This is created by the `Future::then_yield` method.
#[must_use = "futures do nothing unless polled"]
pub struct ThenYield<A> where A: Future {
    state: State<A>,
}

pub fn new<A>(future: A) -> ThenYield<A>
    where A: Future
{
    ThenYield {
        state: State::NotReady(future),
    }
}

impl<A> Future for ThenYield<A>
    where A: Future,
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self) -> Poll<A::Item, A::Error> {
        self.state = match self.state {
            State::NotReady(ref mut future) => match future.poll() {
                poll @ Ok(Async::NotReady) => return poll,
                poll => State::Ready(poll),
            },
            State::Ready(ref mut poll) => {
                return mem::replace(poll, Ok(Async::NotReady));
            },
        };
        task::park().unpark(); // "yield"
        Ok(Async::NotReady)
    }
}
