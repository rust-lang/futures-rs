use std::marker;

use {Future, FutureError, Callback, PollResult, FutureResult, PollError};

pub struct Empty<T, E> {
    canceled: bool,
    _marker: marker::PhantomData<(T, E)>,
}

pub fn empty<T: Send + 'static, E: Send + 'static>() -> Empty<T, E> {
    Empty { canceled: false, _marker: marker::PhantomData }
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

    fn await(&mut self) -> FutureResult<T, E> {
        if self.canceled {
            Err(FutureError::Canceled)
        } else {
            panic!("cannot ever successfully await() on Empty")
        }
    }

    fn cancel(&mut self) {
        self.canceled = true;
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<T, E>) + Send + 'static
    {
        drop(g);
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        drop(cb);
    }
}

impl<T, E> Clone for Empty<T, E> {
    fn clone(&self) -> Empty<T, E> {
        Empty { canceled: self.canceled, _marker: marker::PhantomData }
    }
}

