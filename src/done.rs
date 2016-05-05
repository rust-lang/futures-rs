use {PollResult, Future, IntoFuture, PollError, Callback};
use util;

#[derive(Clone)]
pub struct Done<T, E> {
    inner: Option<Result<T, E>>,
}

pub fn done<T, E>(r: Result<T, E>) -> Done<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Done { inner: Some(r) }
}

impl<T, E> Done<T, E> {
    fn take(&mut self) -> PollResult<T, E> {
        util::opt2poll(self.inner.take()).and_then(|r| {
            r.map_err(PollError::Other)
        })
    }
}

impl<T, E> IntoFuture for Result<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Future = Done<T, E>;
    type Item = T;
    type Error = E;

    fn into_future(self) -> Done<T, E> {
        done(self)
    }
}

impl<T, E> Future for Done<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    // fn poll(&mut self) -> Option<PollResult<T, E>> {
    //     Some(self.take())
    // }

    // fn await(&mut self) -> FutureResult<T, E> {
    //     Ok(try!(self.take()))
    // }

    fn cancel(&mut self) {
        // noop, "already done"
    }

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(PollResult<T, E>) + Send + 'static
    {
        f(self.take())
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        self.schedule(|r| cb.call(r));
    }
}
