use std::sync::Arc;

use {Future, Wake, Tokens, PollError, PollResult};
use stream::Stream;
use util;

pub struct ForEach<S, F> {
    stream: S,
    f: Option<F>,
}

pub fn new<S, F>(s: S, f: F) -> ForEach<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Result<(), S::Error> + Send + 'static
{
    ForEach {
        stream: s,
        f: Some(f),
    }
}

impl<S, F> Future for ForEach<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Result<(), S::Error> + Send + 'static
{
    type Item = ();
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<PollResult<(), S::Error>> {
        loop {
            let item = match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => e,
                Some(Ok(None)) => return Some(Ok(())),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            };
            let mut f = match util::opt2poll(self.f.take()) {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            match util::recover(move || (f(item), f)) {
                Ok((res, f)) => {
                    self.f = Some(f);
                    if let Err(e) = res {
                        return Some(Err(PollError::Other(e)));
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
