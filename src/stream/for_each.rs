use std::sync::Arc;

use {Future, Wake, Tokens, TOKENS_ALL, Poll};
use stream::Stream;

/// A stream combinator which executes a unit closure over each item on a
/// stream.
///
/// This structure is returned by the `Stream::for_each` method.
pub struct ForEach<S, F> {
    stream: S,
    f: F,
}

pub fn new<S, F>(s: S, f: F) -> ForEach<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Result<(), S::Error> + Send + 'static
{
    ForEach {
        stream: s,
        f: f,
    }
}

impl<S, F> Future for ForEach<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Result<(), S::Error> + Send + 'static
{
    type Item = ();
    type Error = S::Error;

    fn poll(&mut self, mut tokens: &Tokens) -> Poll<(), S::Error> {
        loop {
            match try_poll!(self.stream.poll(tokens)) {
                Ok(Some(e)) => {
                    match (self.f)(e) {
                        Ok(()) => {}
                        Err(e) => return Poll::Err(e),
                    }
                }
                Ok(None) => return Poll::Ok(()),
                Err(e) => return Poll::Err(e),
            }
            tokens = &TOKENS_ALL;
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
