use executor::{Executor, DEFAULT};
use util;
use {PollResult, Future, PollError, Callback};

pub struct Done<T, E> {
    inner: Option<Result<T, E>>,
}

pub fn done<T, E>(r: Result<T, E>) -> Done<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Done { inner: Some(r) }
}

impl<T, E> Future for Done<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(PollResult<T, E>) + Send + 'static
    {
        let res = util::opt2poll(self.inner.take()).and_then(|r| {
            r.map_err(PollError::Other)
        });
        DEFAULT.execute(|| f(res))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        self.schedule(|r| cb.call(r));
    }
}
