use std::sync::Arc;

use {Future, Error2, Callback, FutureResult, FutureError};
use slot::Slot;

pub struct Promise<T, E> {
    slot: Arc<Slot<Result<T, E>>>,
}

pub struct Complete<T, E> {
    slot: Arc<Slot<Result<T, E>>>,
}

pub fn pair<T, E>() -> (Promise<T, E>, Complete<T, E>)
    where T: Send + 'static,
          E: Error2,
{
    let slot = Arc::new(Slot::new(None));
    (Promise { slot: slot.clone() }, Complete { slot: slot })
}

impl<T: Send + 'static, E: Error2> Complete<T, E> {
    pub fn finish(self, t: T) {
        self.complete(Ok(t))
    }

    pub fn fail(self, e: E) {
        self.complete(Err(e))
    }

    fn complete(self, t: Result<T, E>) {
        if let Err(e) = self.slot.try_produce(t) {
            self.slot.on_empty(|slot| {
                slot.try_produce(e.into_inner()).ok().unwrap();
            })
        }
    }
}

impl<T: Send + 'static, E: Error2> Future for Promise<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<FutureResult<T, E>> {
        self.slot.try_consume().map(|r| {
            r.map_err(FutureError::Other)
        }).ok()
    }

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(FutureResult<T, E>) + Send + 'static
    {
        self.slot.on_full(move |slot| {
            match slot.try_consume() {
                Ok(data) => f(data.map_err(FutureError::Other)),
                Err(_) => panic!("slot wasn't full on full"),
            }
        })
    }

    fn schedule_boxed(&mut self, f: Box<Callback<T, E>>) {
        self.schedule(|r| f.call(r))
    }
}
