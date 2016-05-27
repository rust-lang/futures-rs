use {Future, Callback, PollResult, PollError};
use executor::{Executor, DEFAULT};
use util;

pub struct Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    callback: Option<Box<Callback<T, E>>>,
}

pub fn empty<T: Send + 'static, E: Send + 'static>() -> Empty<T, E> {
    Empty {
        callback: None,
    }
}

impl<T, E> Future for Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<T, E>) + Send + 'static
    {
        self.schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        if self.callback.is_some() {
            DEFAULT.execute(|| cb.call(Err(util::reused())))
        } else {
            self.callback = Some(cb);
        }
    }
}

impl<T, E> Drop for Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        if let Some(cb) = self.callback.take() {
            DEFAULT.execute(|| cb.call(Err(PollError::Canceled)));
        }
    }
}
