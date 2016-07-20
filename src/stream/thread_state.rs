use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};

pub struct ThreadState<S, State, F> {
    stream: S,
    state: State,
    f: F,
}

pub fn new<S, State, F, U>(s: S, state: State, f: F) -> ThreadState<S, State, F>
    where S: Stream,
          F: FnMut(&mut State, S::Item) -> U + Send + 'static,
          U: Send + 'static,
          State: Send + 'static
{
    ThreadState {
        stream: s,
        state: state,
        f: f,
    }
}

impl<S, State, F, U> Stream for ThreadState<S, State, F>
    where S: Stream,
          F: FnMut(&mut State, S::Item) -> U + Send + 'static,
          U: Send + 'static,
          State: Send + 'static
{
    type Item = U;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<StreamResult<U, S::Error>> {
        match self.stream.poll(tokens) {
            Some(Ok(Some(e))) => Some(Ok(Some((self.f)(&mut self.state, e)))),
            Some(Ok(None)) => Some(Ok(None)),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
