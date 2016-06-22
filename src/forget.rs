use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use {Future, Wake};
use executor::{DEFAULT, Executor};
use slot::Slot;

type Thunk = Box<Future<Item=(), Error=()>>;

struct Forget {
    slot: Slot<(Thunk, Arc<Forget>)>,
    registered: AtomicBool,
    generation: AtomicUsize,
}

pub fn forget<T: Future>(mut t: T) {
    if t.poll().is_some() {
        return
    }
    let thunk = t.map(|_| ()).map_err(|_| ()).boxed();
    _forget(thunk, Arc::new(Forget {
        slot: Slot::new(None),
        registered: AtomicBool::new(false),
        generation: AtomicUsize::new(0),
    }))
}

fn _forget(mut future: Thunk, forget: Arc<Forget>) {
    if future.poll().is_some() {
        return
    }
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
