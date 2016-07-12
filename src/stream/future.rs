use std::sync::Arc;

use {Tokens, Wake, Future};
use stream::Stream;

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
            -> Option<Result<Self::Item, Self::Error>> {
        let item = match self.stream {
            Some(ref mut s) => {
                match s.poll(tokens) {
                    Some(Ok(e)) => e,
                    Some(Err(e)) => return Some(Err(e)),
                    None => return None,
                }
            }
            None => panic!("cannot poll StreamFuture twice"),
        };

        Some(Ok((item, self.stream.take().unwrap())))
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if let Some(s) = self.stream.as_mut() {
            s.schedule(wake)
        }
    }
}
