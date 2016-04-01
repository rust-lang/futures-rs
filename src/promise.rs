use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use {Future, Callback, PollResult, PollError};
use slot::Slot;
use util;

pub struct Promise<T, E> {
    inner: Arc<Inner<T, E>>,
    used: bool,
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
    ready: AtomicBool,
}

pub fn promise<T, E>() -> (Promise<T, E>, Complete<T, E>)
    where T: Send + 'static,
          E: Send + 'static,
{
    let inner = Arc::new(Inner {
        slot: Slot::new(None),
        ready: AtomicBool::new(false),
    });
    (Promise { inner: inner.clone(), used: false },
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
        if self.inner.ready.swap(true, Ordering::SeqCst) {
            return
        }
        if let Err(e) = self.inner.slot.try_produce(t) {
            self.inner.slot.on_empty(|slot| {
                slot.try_produce(e.into_inner()).ok()
                    .expect("advertised as empty but wasn't");
            })
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

    fn poll(&mut self) -> Option<PollResult<T, E>> {
        if self.used {
            return Some(Err(util::reused()))
        }
        self.inner.slot.try_consume().map(|r| {
            self.used = true;
            match r {
                Some(Ok(e)) => Ok(e),
                Some(Err(e)) => Err(PollError::Other(e)),
                None => Err(PollError::Canceled),
            }
        }).ok()
    }

    fn cancel(&mut self) {
        if !self.inner.ready.swap(true, Ordering::SeqCst) {
            self.inner.slot.try_produce(None).ok()
                .expect("got cancel lock but couldn't produce");
        }
    }

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(PollResult<T, E>) + Send + 'static
    {
        if self.used {
            return f(Err(util::reused()))
        }
        self.used = true;
        self.inner.slot.on_full(move |slot| {
            match slot.try_consume() {
                Ok(Some(Ok(e))) => f(Ok(e)),
                Ok(Some(Err(e))) => f(Err(PollError::Other(e))),
                Ok(None) => f(Err(PollError::Canceled)),
                Err(..) => panic!("slot wasn't full"),
            }
        })
    }

    fn schedule_boxed(&mut self, f: Box<Callback<T, E>>) {
        self.schedule(|r| f.call(r))
    }
}
