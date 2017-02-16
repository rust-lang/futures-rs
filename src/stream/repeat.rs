use core::marker;

use stream::Stream;
use task::Task;

use {Async, Poll};


/// Stream that produces the same element repeatedly.
#[must_use = "streams do nothing unless polled"]
pub struct Repeat<T, E>
    where T: Clone
{
    item: T,
    error: marker::PhantomData<E>,
}

/// Create a stream which produces the same item repeatedly.
///
/// Stream never produces an error or EOF.
///
/// ```rust
/// use futures::*;
///
/// let mut stream = stream::repeat::<_, bool>(10);
/// assert_eq!(Ok(Async::Ready(Some(10))), stream.poll(&task::empty()));
/// assert_eq!(Ok(Async::Ready(Some(10))), stream.poll(&task::empty()));
/// assert_eq!(Ok(Async::Ready(Some(10))), stream.poll(&task::empty()));
/// ```
pub fn repeat<T, E>(item: T) -> Repeat<T, E>
    where T: Clone
{
    Repeat {
        item: item,
        error: marker::PhantomData,
    }
}

impl<T, E> Stream for Repeat<T, E>
    where T: Clone
{
    type Item = T;
    type Error = E;

    fn poll(&mut self, _task: &Task) -> Poll<Option<Self::Item>, Self::Error> {
        Ok(Async::Ready(Some(self.item.clone())))
    }
}
