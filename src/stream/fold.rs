use std::sync::Arc;

use {Wake, Tokens, Future, PollResult};
use stream::Stream;
use util;

pub struct Fold<S, F, T> {
    stream: S,
    state: Option<(F, T)>,
}

pub fn new<S, F, T>(s: S, f: F, t: T) -> Fold<S, F, T>
    where S: Stream,
          F: FnMut(T, S::Item) -> T + Send + 'static,
          T: Send + 'static,
{
    Fold {
        stream: s,
        state: Some((f, t)),
    }
}

impl<S, F, T> Future for Fold<S, F, T>
    where S: Stream,
          F: FnMut(T, S::Item) -> T + Send + 'static,
          T: Send + 'static,
{
    type Item = T;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<T, S::Error>> {
        loop {
            match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => {
                    let (mut f, t) = match util::opt2poll(self.state.take()) {
                        Ok(s) => s,
                        Err(e) => return Some(Err(e)),
                    };
                    match util::recover(|| (f(t, e), f)) {
                        Ok((next, f)) => self.state = Some((f, next)),
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(None)) => {
                    return Some(util::opt2poll(self.state.take().map(|p| p.1)))
                }
                Some(Err(e)) => {
                    drop(self.state.take());
                    return Some(Err(e))
                }
                None => return None,
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
