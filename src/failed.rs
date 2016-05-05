use std::marker;

use {Future, PollResult, PollError, Callback};
use util;

pub struct Failed<T, E> {
    _t: marker::PhantomData<T>,
    e: Option<E>,
}

pub fn failed<T, E>(e: E) -> Failed<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Failed { _t: marker::PhantomData, e: Some(e) }
}

impl<T, E> Future for Failed<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    // fn poll(&mut self) -> Option<PollResult<T, E>> {
    //     Some(util::opt2poll(self.e.take()).and_then(|e| {
    //         Err(PollError::Other(e))
    //     }))
    // }

    // fn await(&mut self) -> FutureResult<T, E> {
    //     Ok(try!(self.poll().unwrap()))
    // }

    // fn cancel(&mut self) {
    //     // noop, already done
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<T, E>) + Send + 'static
    {
        g(util::opt2poll(self.e.take()).and_then(|e| Err(PollError::Other(e))))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        self.schedule(|r| cb.call(r))
    }
}
