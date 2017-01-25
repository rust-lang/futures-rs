use core;

use Poll;
use stream;
use stream::Stream;
use task::Task;

/// A stream which emits single element and then EOF.
///
/// This stream will never block and is always ready.
#[must_use = "streams do nothing unless polled"]
pub struct Once<T, E>(stream::IterStream<core::iter::Once<Result<T, E>>>);

/// Creates a stream of single element
///
/// ```rust
/// use futures::*;
///
/// let mut stream = stream::once::<(), _>(Err(17));
/// assert_eq!(Err(17), stream.poll(&task::empty()));
/// assert_eq!(Ok(Async::Ready(None)), stream.poll(&task::empty()));
/// ```
pub fn once<T, E>(item: Result<T, E>) -> Once<T, E> {
    Once(stream::iter(core::iter::once(item)))
}

impl<T, E> Stream for Once<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self, task: &Task) -> Poll<Option<T>, E> {
        self.0.poll(task)
    }
}
