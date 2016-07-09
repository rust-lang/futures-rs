use std::sync::Arc;

use {Tokens, Wake, Future, PollResult};
use stream::Stream;
use util;

pub struct StreamFuture<S> {
    stream: Option<S>,
}

pub fn new<S: Stream>(s: S) -> StreamFuture<S> {
    StreamFuture { stream: Some(s) }
}

impl<S: Stream> Future for StreamFuture<S> {
    type Item = (Option<S::Item>, S);
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<PollResult<Self::Item, Self::Error>> {
        let item = match self.stream {
            Some(ref mut s) => {
                match s.poll(tokens) {
                    Some(Ok(e)) => e,
                    Some(Err(e)) => return Some(Err(e)),
                    None => return None,
                }
            }
            None => return Some(Err(util::reused())),
        };

        Some(Ok((item, self.stream.take().unwrap())))
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if let Some(s) = self.stream.as_mut() {
            s.schedule(wake)
        }
    }
}
