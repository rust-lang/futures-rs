use std::sync::Arc;

use {Future, Wake, Tokens};
use stream::Stream;

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

    fn poll(&mut self, tokens: &Tokens) -> Option<Result<(), S::Error>> {
        loop {
            // TODO: reset the tokens on each turn?
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
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
