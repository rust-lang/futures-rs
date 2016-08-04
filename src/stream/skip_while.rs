use {Task, Poll, IntoFuture, Future};
use stream::Stream;

/// A stream combinator which skips elements of a stream while a predicate
/// holds.
///
/// This structure is produced by the `Stream::skip_while` method.
pub struct SkipWhile<S, P, R> where S: Stream, R: IntoFuture {
    stream: S,
    pred: P,
    pending: Option<(R::Future, S::Item)>,
    done_skipping: bool,
}

pub fn new<S, P, R>(s: S, p: P) -> SkipWhile<S, P, R>
    where S: Stream,
          P: FnMut(&S::Item) -> R + Send + 'static,
          R: IntoFuture<Item=bool, Error=S::Error>,
{
    SkipWhile {
        stream: s,
        pred: p,
        pending: None,
        done_skipping: false,
    }
}

impl<S, P, R> Stream for SkipWhile<S, P, R>
    where S: Stream,
          P: FnMut(&S::Item) -> R + Send + 'static,
          R: IntoFuture<Item=bool, Error=S::Error>,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<S::Item>, S::Error> {
        if self.done_skipping {
            return self.stream.poll(task);
        }

        loop {
            if self.pending.is_none() {
                let item = match try_poll!(self.stream.poll(task)) {
                    Ok(Some(e)) => e,
                    Ok(None) => return Poll::Ok(None),
                    Err(e) => return Poll::Err(e),
                };
                self.pending = Some(((self.pred)(&item).into_future(), item));
            }

            assert!(self.pending.is_some());
            match try_poll!(self.pending.as_mut().unwrap().0.poll(task)) {
                Ok(true) => self.pending = None,
                Ok(false) => {
                    let (_, item) = self.pending.take().unwrap();
                    self.done_skipping = true;
                    return Poll::Ok(Some(item))
                }
                Err(e) => {
                    self.pending = None;
                    return Poll::Err(e)
                }
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        self.stream.schedule(task)
    }
}

impl<S, P, R> SkipWhile<S, P, R>
    where S: Stream,
          P: FnMut(&S::Item) -> R + Send + 'static,
          R: IntoFuture<Item=bool, Error=S::Error>,
{
    /// Consume this adaptor, returning the underlying stream.
    ///
    /// Note that if an element is buffered or a future is active determining
    /// whether that element should be yielded they will both be dropped as part
    /// of this operation.
    pub fn into_inner(self) -> S {
        self.stream
    }
}
