use std::marker;

use {Future, Callback, PollResult, PollError};
use util;

pub struct Empty<T, E> {
    canceled: bool,
    callback: Option<Box<Callback<T, E>>>,
    _marker: marker::PhantomData<(T, E)>,
}

pub fn empty<T: Send + 'static, E: Send + 'static>() -> Empty<T, E> {
    Empty {
        canceled: false,
        callback: None,
        _marker: marker::PhantomData,
    }
}

impl<T, E> Future for Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<PollResult<T, E>> {
        if self.canceled {
            Some(Err(PollError::Canceled))
        } else {
            None
        }
    }

    // fn await(&mut self) -> FutureResult<T, E> {
    //     if self.canceled {
    //         Err(FutureError::Canceled)
    //     } else {
    //         panic!("cannot ever successfully await() on Empty")
    //     }
    // }

    fn cancel(&mut self) {
        self.canceled = true;
        if let Some(cb) = self.callback.take() {
            cb.call(Err(PollError::Canceled))
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<T, E>) + Send + 'static
    {
        self.schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        if self.canceled {
            cb.call(Err(PollError::Canceled));
        } else if self.callback.is_some() {
            cb.call(Err(util::reused()))
        } else {
            self.callback = Some(cb);
        }
    }
}
