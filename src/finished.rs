use std::marker;

use {Future, PollResult, Callback};
use executor::{Executor, DEFAULT};
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

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<T, E>) + Send + 'static
    {
        let res = util::opt2poll(self.t.take());
        DEFAULT.execute(|| g(res));
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        self.schedule(|r| cb.call(r))
    }
}
