use std::any::Any;

pub type PollResult<T, E> = Result<T, PollError<E>>;

pub enum PollError<E> {
    Panicked(Box<Any + Send>),
    Canceled,
    Other(E),
}

impl<E> PollError<E> {
    pub fn map<F: FnOnce(E) -> E2, E2>(self, f: F) -> PollError<E2> {
        match self {
            PollError::Panicked(e) => PollError::Panicked(e),
            PollError::Canceled => PollError::Canceled,
            PollError::Other(e) => PollError::Other(f(e)),
        }
    }
}
