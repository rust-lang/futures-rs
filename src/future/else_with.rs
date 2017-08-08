use {Async, Future, Poll};

/// Future for the `else_with` combinator, passing state along into the result
/// of a future which produces an error.
///
/// This is created by the `Future::else_with` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ElseWith<A, T> where A: Future {
    future: A,
    context: Option<T>,
}

pub fn new<A, T>(future: A, context: T) -> ElseWith<A, T>
    where A: Future,
{
    ElseWith {
        future: future,
        context: Some(context),
    }
}

impl<A, T> Future for ElseWith<A, T>
    where A: Future,
{
    type Item = A::Item;
    type Error = (A::Error, T);

    fn poll(&mut self) -> Poll<A::Item, (A::Error, T)> {
        match self.future.poll() {
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Ok(Async::Ready(item)) => Ok(item.into()),
            Err(err) => {
                let context = self.context.take().expect("ElseWith polled after complete");
                Err((err, context))
            }
        }
    }
}
