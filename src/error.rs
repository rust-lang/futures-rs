use std::any::Any;

pub type FutureResult<T, E> = Result<T, FutureError<E>>;
pub type PollResult<T, E> = Result<T, PollError<E>>;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FutureError<E> {
    Canceled,
    Other(E),
}

pub enum PollError<E> {
    Panicked(Box<Any + Send>),
    Canceled,
    Other(E),
}

impl<E> FutureError<E> {
    pub fn map<E2, F: FnOnce(E) -> E2>(self, f: F) -> FutureError<E2> {
        match self {
            FutureError::Canceled => FutureError::Canceled,
            FutureError::Other(e) => FutureError::Other(f(e)),
        }
    }
}

impl<E> From<FutureError<E>> for PollError<E> {
    fn from(other: FutureError<E>) -> PollError<E> {
        match other {
            FutureError::Canceled => PollError::Canceled,
            FutureError::Other(e) => PollError::Other(e),
        }
    }
}

impl<E> From<PollError<E>> for FutureError<E> {
    fn from(other: PollError<E>) -> FutureError<E> {
        match other {
            PollError::Panicked(b) => panic!(b), // TODO: propagate
            PollError::Canceled => FutureError::Canceled,
            PollError::Other(e) => FutureError::Other(e),
        }
    }
}
