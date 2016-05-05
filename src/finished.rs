use std::marker;

use {Future, PollResult, Callback};
use util;

pub struct Finished<T, E> {
    t: Option<T>,
    _e: marker::PhantomData<E>,
}

pub fn finished<T, E>(t: T) -> Finished<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Finished { t: Some(t), _e: marker::PhantomData }
}

impl<T, E> Future for Finished<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    // fn poll(&mut self) -> Option<PollResult<T, E>> {
    //     Some(util::opt2poll(self.t.take()))
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
        g(util::opt2poll(self.t.take()))
        // g(self.poll().unwrap())
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        self.schedule(|r| cb.call(r))
    }
}
