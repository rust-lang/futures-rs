use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use {Future, Wake, PollResult};
use executor::{DEFAULT, Executor};
use slot::Slot;

type Thunk = Box<Future<Item=(), Error=()>>;

struct Forget {
    slot: Slot<(Thunk, Arc<Forget>)>,
    registered: AtomicBool,
}

pub fn forget<T: Future>(mut t: T) {
    if t.poll().is_some() {
        return
    }
    let thunk = ThunkFuture { inner: t.boxed() }.boxed();
    _forget(thunk, Arc::new(Forget {
        slot: Slot::new(None),
        registered: AtomicBool::new(false),
    }))
}

// FIXME(rust-lang/rust#34416) should just be able to use map/map_err, but that
//                             causes trans to go haywire.
struct ThunkFuture<T, E> {
    inner: Box<Future<Item=T, Error=E>>,
}

impl<T: Send + 'static, E: Send + 'static> Future for ThunkFuture<T, E> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Option<PollResult<(), ()>> {
        match self.inner.poll() {
            Some(Ok(_)) => Some(Ok(())),
            Some(Err(e)) => Some(Err(e.map(|_| ()))),
            None => None,
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.inner.schedule(wake)
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=(), Error=()>>> {
        if let Some(f) = self.inner.tailcall() {
            self.inner = f;
        }
        None
    }
}

fn _forget(mut future: Thunk, forget: Arc<Forget>) {
    if future.poll().is_some() {
        return
    }
    let mut future = match future.tailcall() {
        Some(f) => f,
        None => future,
    };
    future.schedule(forget.clone());
    forget.slot.try_produce((future, forget.clone())).ok().unwrap();
}

impl Wake for Forget {
    fn wake(&self) {
        if self.registered.swap(true, Ordering::SeqCst) {
            return
        }
        self.slot.on_full(|slot| {
            let (future, forget) = slot.try_consume().ok().unwrap();
            forget.registered.store(false, Ordering::SeqCst);
            DEFAULT.execute(|| _forget(future, forget))
        });
    }
}
