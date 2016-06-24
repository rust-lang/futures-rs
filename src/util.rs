use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

use {PollResult, PollError, Wake, Future, Tokens};
use executor::{Executor, DEFAULT};

pub enum Collapsed<T: Future> {
    Start(T),
    Tail(Box<Future<Item=T::Item, Error=T::Error>>),
}

// TODO: reexport this?
struct ReuseFuture;

/// Wrapper around panic::catch_unwind which assumes that `Send + 'static` types
/// are `UnwindSafe`.
///
/// Returns a `PollResult` to interoperate with this library and
/// `Err(PollError::Panicked(..))` is only produced if the provided closure
/// panics.
pub fn recover<F, R, E>(f: F) -> PollResult<R, E>
    where F: FnOnce() -> R + Send + 'static
{
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(PollError::Panicked)
}

/// Produces a `PollError::Panicked` indicating that a future was reused when it
/// should not have been (e.g. schedule was called twice).
pub fn reused<E>() -> PollError<E> {
    PollError::Panicked(Box::new(ReuseFuture))
}

/// Attempts to unwrap an option, returning a `PollError::Panicked` if the
/// option is `None`.
///
/// This is useful for futures who internall have mutable state stored in an
/// `Option<T>`.
pub fn opt2poll<T, E>(t: Option<T>) -> PollResult<T, E> {
    match t {
        Some(t) => Ok(t),
        None => Err(reused()),
    }
}

impl<T: Future> Collapsed<T> {
    pub fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<T::Item, T::Error>> {
        match *self {
            Collapsed::Start(ref mut a) => a.poll(tokens),
            Collapsed::Tail(ref mut a) => a.poll(tokens),
        }
    }

    pub fn schedule(&mut self, wake: Arc<Wake>) -> Tokens {
        match *self {
            Collapsed::Start(ref mut a) => a.schedule(wake),
            Collapsed::Tail(ref mut a) => a.schedule(wake),
        }
    }

    pub fn collapse(&mut self) {
        let a = match *self {
            Collapsed::Start(ref mut a) => {
                match a.tailcall() {
                    Some(a) => a,
                    None => return,
                }
            }
            Collapsed::Tail(ref mut a) => {
                if let Some(b) = a.tailcall() {
                    *a = b;
                }
                return
            }
        };
        *self = Collapsed::Tail(a);
    }
}

pub fn done(wake: Arc<Wake>) -> Tokens {
    DEFAULT.execute(move || wake.wake(&Tokens::all()));
    Tokens::all()
}
