use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use {Future, Wake, PollResult, Tokens};
use executor::{DEFAULT, Executor};
use slot::Slot;

type Thunk = Box<Future<Item=(), Error=()>>;

struct Forget {
    slot: Slot<(Thunk, Arc<Forget>)>,
    registered: AtomicBool,
    tokens: AtomicUsize,
}

pub fn forget<T: Future>(mut t: T) {
    if t.poll(&Tokens::all()).is_some() {
        return
    }
    let thunk = ThunkFuture { inner: t.boxed() }.boxed();
    let forget = Arc::new(Forget {
        slot: Slot::new(None),
        registered: AtomicBool::new(false),
        tokens: AtomicUsize::new(0),
    });
    _forget(thunk, forget, &Tokens::all())
}

// FIXME(rust-lang/rust#34416) should just be able to use map/map_err, but that
//                             causes trans to go haywire.
struct ThunkFuture<T, E> {
    inner: Box<Future<Item=T, Error=E>>,
}

impl<T: Send + 'static, E: Send + 'static> Future for ThunkFuture<T, E> {
    type Item = ();
    type Error = ();

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<(), ()>> {
        match self.inner.poll(tokens) {
            Some(Ok(_)) => Some(Ok(())),
            Some(Err(e)) => Some(Err(e.map(|_| ()))),
            None => None,
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) -> Tokens {
        self.inner.schedule(wake)
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=(), Error=()>>> {
        if let Some(f) = self.inner.tailcall() {
            self.inner = f;
        }
        None
    }
}

fn _forget(mut future: Thunk,
           forget: Arc<Forget>,
           tokens: &Tokens) {
    if future.poll(tokens).is_some() {
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
    fn wake(&self, tokens: &Tokens) {
        self.tokens.fetch_or(tokens.as_usize(), Ordering::SeqCst);
        if self.registered.swap(true, Ordering::SeqCst) {
            return
        }
        self.slot.on_full(|slot| {
            let (future, forget) = slot.try_consume().ok().unwrap();

            // TODO: think real hard about the ordering of this store and the
            //       swap below
            forget.registered.store(false, Ordering::SeqCst);
            DEFAULT.execute(|| {
                let tokens = forget.tokens.swap(0, Ordering::SeqCst);
                let tokens = Tokens::from_usize(tokens);
                _forget(future, forget, &tokens)
            })
        });
    }
}
