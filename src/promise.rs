use std::mem;
use std::sync::Arc;

use {Future, Callback, PollResult, PollError};
use slot::{Slot, Token};
use util;

pub struct Promise<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    state: _Promise<T, E>,
}

enum _Promise<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Start(Arc<Inner<T, E>>),
    Scheduled(Arc<Inner<T, E>>, Token),
    Canceled,
    Used,
}

pub struct Complete<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: Arc<Inner<T, E>>,
    completed: bool,
}

struct Inner<T, E> {
    slot: Slot<Option<Result<T, E>>>,
}

pub fn promise<T, E>() -> (Promise<T, E>, Complete<T, E>)
    where T: Send + 'static,
          E: Send + 'static,
{
    let inner = Arc::new(Inner {
        slot: Slot::new(None),
    });
    (Promise { state: _Promise::Start(inner.clone()) },
     Complete { inner: inner, completed: false })
}

impl<T, E> Complete<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    pub fn finish(mut self, t: T) {
        self.completed = true;
        self.complete(Some(Ok(t)))
    }

    pub fn fail(mut self, e: E) {
        self.completed = true;
        self.complete(Some(Err(e)))
    }

    fn complete(&mut self, t: Option<Result<T, E>>) {
        if let Err(e) = self.inner.slot.try_produce(t) {
            self.inner.slot.on_empty(|slot| {
                slot.try_produce(e.into_inner()).ok()
                    .expect("advertised as empty but wasn't");
            });
        }
    }
}

impl<T, E> Drop for Complete<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        if !self.completed {
            self.complete(None);
        }
    }
}

impl<T: Send + 'static, E: Send + 'static> Future for Promise<T, E> {
    type Item = T;
    type Error = E;

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(PollResult<T, E>) + Send + 'static
    {
        let inner = match mem::replace(&mut self.state, _Promise::Used) {
            _Promise::Start(inner) => inner,
            _Promise::Canceled => return f(Err(PollError::Canceled)),
            _Promise::Used => return f(Err(util::reused())),
            _Promise::Scheduled(s, token) => {
                self.state = _Promise::Scheduled(s, token);
                return f(Err(util::reused()))
            }
        };
        let token = inner.slot.on_full(|slot| {
            match slot.try_consume() {
                Ok(Some(Ok(e))) => f(Ok(e)),
                Ok(Some(Err(e))) => f(Err(PollError::Other(e))),

                // canceled because the `Complete` handle dropped
                Ok(None) => f(Err(PollError::Canceled)),

                // canceled via Drop
                Err(..) => f(Err(PollError::Canceled)),
            }
        });
        self.state = _Promise::Scheduled(inner, token);
    }

    fn schedule_boxed(&mut self, f: Box<Callback<T, E>>) {
        self.schedule(|r| f.call(r))
    }
}

impl<T, E> Drop for Promise<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        match mem::replace(&mut self.state, _Promise::Canceled) {
            _Promise::Start(..) => {}
            _Promise::Canceled => {}
            _Promise::Used => {}
            _Promise::Scheduled(s, token) => {
                s.slot.cancel(token);
            }
        }
    }
}
