//! Definition of the `Option` (optional step) combinator

use {Future, Poll, Async};
use task::Task;

impl<F, T, E> Future for Option<F> where F: Future<Item=T, Error=E> {
    type Item = Option<T>;
    type Error = E;

    fn poll(&mut self, task: &Task) -> Poll<Option<T>, E> {
        match *self {
            None => Ok(Async::Ready(None)),
            Some(ref mut x) => x.poll(task).map(|x| x.map(Some)),
        }
    }
}
