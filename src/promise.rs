use std::sync::Arc;

use Future;
use slot::Slot;

pub struct Promise<T, E> {
    slot: Arc<Slot<Result<T, E>>>,
}

pub struct Complete<T, E> {
    slot: Arc<Slot<Result<T, E>>>,
}

pub fn pair<T, E>() -> (Promise<T, E>, Complete<T, E>)
    where T: Send + 'static,
          E: Send + 'static,
{
    let slot = Arc::new(Slot::new(None));
    (Promise { slot: slot.clone() }, Complete { slot: slot })
}

impl<T: Send + 'static, E: Send + 'static> Complete<T, E> {
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

impl<T: Send + 'static, E: Send + 'static> Future for Promise<T, E> {
    type Item = T;
    type Error = E;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        match self.slot.try_consume() {
            Ok(val) => Ok(val),
            Err(..) => Err(self),
        }
    }

    fn schedule<F>(self, f: F)
        where F: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
    {
        self.slot.on_full(move |slot| {
            match slot.try_consume() {
                Ok(data) => f(data),
                Err(_) => panic!("slot wasn't full on full"),
            }
        })
    }
}
