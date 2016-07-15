use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use {Future, Wake, Tokens};
use token::AtomicTokens;
use executor::{DEFAULT, Executor};
use slot::Slot;

type Thunk = Box<Future<Item=(), Error=()>>;

struct Forget {
    slot: Slot<(Thunk, Arc<Forget>, Arc<Wake>)>,
    registered: AtomicBool,
    tokens: AtomicTokens,
}

pub fn forget<T: Future>(t: T) {
    let thunk = ThunkFuture { inner: t.boxed() }.boxed();
    let forget = Arc::new(Forget {
        slot: Slot::new(None),
        registered: AtomicBool::new(false),
        tokens: AtomicTokens::all(),
    });
    _forget(thunk, forget.clone(), forget)
}

// FIXME(rust-lang/rust#34416) should just be able to use map/map_err, but that
//                             causes trans to go haywire.
struct ThunkFuture<T, E> {
    inner: Box<Future<Item=T, Error=E>>,
}

impl<T: Send + 'static, E: Send + 'static> Future for ThunkFuture<T, E> {
    type Item = ();
    type Error = ();

    fn poll(&mut self, tokens: &Tokens) -> Option<Result<(), ()>> {
        match self.inner.poll(tokens) {
            Some(Ok(_)) => Some(Ok(())),
            Some(Err(_)) => Some(Err(())),
            None => None,
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
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
           wake: Arc<Wake>) {
    loop {
        // TODO: catch panics here?

        // Note that we need to poll at least once as the wake callback may have
        // received an empty set of tokens, but that's still a valid reason to
        // poll a future.
        if future.poll(&forget.tokens.get_tokens()).is_some() {
            return
        }
        future = match future.tailcall() {
            Some(f) => f,
            None => future,
        };
        if !forget.tokens.any() {
            break
        }
    }

    // Ok, we've seen that there are no tokens which show interest in the
    // future. Schedule interest on the future for when something is ready and
    // then relinquish the future and the forget back to the slot, which will
    // then pick it up once a wake callback has fired.
    future.schedule(&wake);
    forget.slot.try_produce((future, forget.clone(), wake)).ok().unwrap();
}

impl Wake for Forget {
    fn wake(&self, tokens: &Tokens) {
        // First, add all our tokens provided into the shared token set.
        self.tokens.add(tokens);

        // Next, see if we can actually register an `on_full` callback. The
        // `Slot` requires that only one registration happens, and this flag
        // guards that.
        if self.registered.swap(true, Ordering::SeqCst) {
            return
        }

        // If we won the race to register a callback, do so now. Once the slot
        // is resolve we allow another registration **before we poll again**.
        // This allows any future which may be somewhat badly behaved to be
        // compatible with this.
        //
        // TODO: this store of `false` should *probably* be before the
        //       `schedule` call in forget above, need to think it through.
        self.slot.on_full(|slot| {
            let (future, forget, wake) = slot.try_consume().ok().unwrap();
            forget.registered.store(false, Ordering::SeqCst);
            DEFAULT.execute(|| _forget(future, forget, wake));
        });
    }
}
