//! Definition of the `Option` (optional step) combinator

use {Future, Poll, Async};

impl<F, T, E> Future for Option<F> where F: Future<Item=T, Error=E> {
    type Item = Option<T>;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<T>, E> {
        match *self {
            None => Ok(Async::Ready(None)),
            Some(ref mut x) => x.poll().map(|x| x.map(Some)),
        }
    }

    #[cfg(feature = "use_std")]
    fn wait(self) -> Result<Option<T>, E> {
        match self {
            None => Ok(None),
            Some(x) => x.wait().map(Some),
        }
    }
}
