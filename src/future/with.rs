use {Async, Future, Poll};

/// Future for the `with` combinator, passing state along into the result of
/// a future regardless of its outcome.
///
/// This is created by the `Future::with` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct With<A, T> where A: Future {
    future: A,
    context: Option<T>,
}

pub fn new<A, T>(future: A, context: T) -> With<A, T>
    where A: Future,
{
    With {
        future: future,
        context: Some(context),
    }
}

impl<A, T> Future for With<A, T>
    where A: Future,
{
    type Item = (A::Item, T);
    type Error = (A::Error, T);

    fn poll(&mut self) -> Poll<(A::Item, T), (A::Error, T)> {
        match self.future.poll() {
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Ok(Async::Ready(item)) => {
                let context = self.context.take().expect("With polled after complete");
                Ok((item, context).into())
            }
            Err(err) => {
                let context = self.context.take().expect("With polled after complete");
                Err((err, context))
            }
        }
    }
}
