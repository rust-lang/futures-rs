use std::sync::Arc;

use Future;
use slot::Slot;

pub struct Promise<T> {
    slot: Arc<Slot<Result<T, Cancel>>>,
}

pub struct Complete<T> {
    slot: Arc<Slot<Result<T, Cancel>>>,
}

pub struct Cancel(());

pub fn pair<T: Send + 'static>() -> (Promise<T>, Complete<T>) {
    let slot = Arc::new(Slot::new(None));
    (Promise { slot: slot.clone() }, Complete { slot: slot })
}

impl<T: Send + 'static> Complete<T> {
    pub fn finish(self, t: T) {
        self.produce(Ok(t));
    }

    pub fn cancel(self) {
        self.produce(Err(Cancel(())));
    }

    fn produce(&self, result: Result<T, Cancel>) {
        if let Err(e) = self.slot.try_produce(result) {
            self.slot.on_empty(|slot| {
                slot.try_produce(e.into_inner()).ok().unwrap();
            })
        }
    }
}

impl<T: Send + 'static> Future for Promise<T> {
    type Item = T;
    type Error = Cancel;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        self.slot.try_consume().map_err(|_| self)
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
