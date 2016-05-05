use {Future, Callback, PollResult, PollError};
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

    // fn poll(&mut self) -> Option<PollResult<T, E>> {
    //     if self.canceled {
    //         Some(Err(PollError::Canceled))
    //     } else {
    //         None
    //     }
    // }

    // fn await(&mut self) -> FutureResult<T, E> {
    //     if self.canceled {
    //         Err(FutureError::Canceled)
    //     } else {
    //         panic!("cannot ever successfully await() on Empty")
    //     }
    // }

    // fn cancel(&mut self) {
    //     self.canceled = true;
    //     if let Some(cb) = self.callback.take() {
    //         cb.call(Err(PollError::Canceled))
    //     }
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<T, E>) + Send + 'static
    {
        self.schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        if self.callback.is_some() {
            cb.call(Err(util::reused()))
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
            cb.call(Err(PollError::Canceled));
        }
    }
}
