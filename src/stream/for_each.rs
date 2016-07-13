use std::sync::Arc;

use {Future, Wake, Tokens, ALL_TOKENS};
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

    fn poll(&mut self, mut tokens: &Tokens) -> Option<Result<(), S::Error>> {
        loop {
            match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => {
                    match (self.f)(e) {
                        Ok(()) => {}
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(None)) => return Some(Ok(())),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
            tokens = &ALL_TOKENS;
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
