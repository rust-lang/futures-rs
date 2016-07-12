use std::sync::Arc;

use {Wake, Tokens, Future};
use stream::Stream;

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

    fn poll(&mut self, tokens: &Tokens) -> Option<Result<T, S::Error>> {
        let mut state = self.state.take().expect("cannot poll Fold twice");
        loop {
            // TODO: reset tokens once we turn the loop?
            match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => state = (self.f)(state, e),
                Some(Ok(None)) => return Some(Ok(state)),
                Some(Err(e)) => return Some(Err(e)),
                None => {
                    self.state = Some(state);
                    return None
                }
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
