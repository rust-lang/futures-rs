use {Future, Poll};

/// Future for the `and_with` combinator, passing state along into the result
/// of a future which completes successfully.
///
/// This is created by the `Future::and_with` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct AndWith<A, T> where A: Future {
    future: A,
    context: Option<T>,
}

pub fn new<A, T>(future: A, context: T) -> AndWith<A, T>
    where A: Future,
{
    AndWith {
        future: future,
        context: Some(context),
    }
}

impl<A, T> Future for AndWith<A, T>
    where A: Future,
{
    type Item = (A::Item, T);
    type Error = A::Error;

    fn poll(&mut self) -> Poll<(A::Item, T), A::Error> {
        let item = try_ready!(self.future.poll());
        let context = self.context.take().expect("AndWith polled after complete");
        Ok((item, context).into())
    }
}
