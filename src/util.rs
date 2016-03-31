use {PollResult, PollError};

// TODO: reexport this?
struct ReuseFuture;

pub fn recover<F, R, E>(f: F) -> PollResult<R, E>
    where F: FnOnce() -> R + Send + 'static
{
    // use std::panic::{recover, AssertRecoverSafe};
    //
    // recover(AssertRecoverSafe(f)).map_err(|_| FutureError::Panicked)
    Ok(f())
}

pub fn opt2poll<T, E>(t: Option<T>) -> PollResult<T, E> {
    match t {
        Some(t) => Ok(t),
        None => Err(PollError::Panicked(Box::new(ReuseFuture))),
    }
}
