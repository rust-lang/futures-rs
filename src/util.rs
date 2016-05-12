use std::panic::{self, AssertUnwindSafe};

use {PollResult, PollError};

// TODO: reexport this?
struct ReuseFuture;

pub fn recover<F, R, E>(f: F) -> PollResult<R, E>
    where F: FnOnce() -> R + Send + 'static
{
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(PollError::Panicked)
}

pub fn reused<E>() -> PollError<E> {
    PollError::Panicked(Box::new(ReuseFuture))
}

pub fn opt2poll<T, E>(t: Option<T>) -> PollResult<T, E> {
    match t {
        Some(t) => Ok(t),
        None => Err(reused()),
    }
}
