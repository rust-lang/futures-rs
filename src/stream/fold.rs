use std::sync::Arc;

use {Wake, Tokens, Future, TOKENS_ALL, Poll};
use stream::Stream;

/// A future used to collect all the results of a stream into one generic type.
///
/// This future is returned by the `Stream::fold` method.
pub struct Fold<S, F, T> {
    stream: S,
    f: F,
    state: Option<T>,
}

pub fn new<S, F, T>(s: S, f: F, t: T) -> Fold<S, F, T>
    where S: Stream,
          F: FnMut(T, S::Item) -> T + Send + 'static,
          T: Send + 'static,
{
    Fold {
        stream: s,
        f: f,
        state: Some(t),
    }
}

impl<S, F, T> Future for Fold<S, F, T>
    where S: Stream,
          F: FnMut(T, S::Item) -> T + Send + 'static,
          T: Send + 'static,
{
    type Item = T;
    type Error = S::Error;

    fn poll(&mut self, mut tokens: &Tokens) -> Poll<T, S::Error> {
        let mut state = self.state.take().expect("cannot poll Fold twice");
        loop {
            match self.stream.poll(tokens) {
                Poll::Ok(Some(e)) => state = (self.f)(state, e),
                Poll::Ok(None) => return Poll::Ok(state),
                Poll::Err(e) => return Poll::Err(e),
                Poll::NotReady => {
                    self.state = Some(state);
                    return Poll::NotReady
                }
            }
            tokens = &TOKENS_ALL;
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
