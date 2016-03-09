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

pub fn pair<T: 'static>() -> (Promise<T>, Complete<T>) {
    let slot = Arc::new(Slot::new(None));
    (Promise { slot: slot.clone() }, Complete { slot: slot })
}

impl<T: 'static> Complete<T> {
    // TODO: these need to handle the error case as the promise could be
    //       getting scheduled to cause interference with `try_produce`
    pub fn finish(self, t: T) {
        assert!(self.slot.try_produce(Ok(t)).is_ok());
    }

    pub fn cancel(self) {
        assert!(self.slot.try_produce(Err(Cancel(()))).is_ok());
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
        let res = self.slot.on_full(move |slot| {
            match slot.try_consume() {
                Ok(data) => f(data),
                Err(_) => panic!("slot wasn't full on full"),
            }
        });
        // TODO: this needs to handle the error case as the promise could be
        // completing to cause interference with `on_full`
        assert!(res.is_ok());
    }
}
