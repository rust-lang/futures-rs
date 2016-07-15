use std::sync::Arc;

use {Tokens, Wake, Future};
use stream::Stream;

/// A combinator used to temporarily convert a stream into a future.
///
/// This future is returned by the `Stream::into_future` method.
pub struct StreamFuture<S> {
    stream: Option<S>,
}

pub fn new<S: Stream>(s: S) -> StreamFuture<S> {
    StreamFuture { stream: Some(s) }
}

impl<S: Stream> Future for StreamFuture<S> {
    type Item = (Option<S::Item>, S);
    type Error = (S::Error, S);

    fn poll(&mut self, tokens: &Tokens)
            -> Option<Result<Self::Item, Self::Error>> {
        let item = match self.stream {
            Some(ref mut s) => {
                match s.poll(tokens) {
                    Some(Ok(e)) => Ok(e),
                    Some(Err(e)) => Err(e),
                    None => return None,
                }
            }
            None => panic!("cannot poll StreamFuture twice"),
        };
        let stream = self.stream.take().unwrap();

        Some(match item {
            Ok(e) => Ok((e, stream)),
            Err(e) => Err((e, stream)),
        })
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        if let Some(s) = self.stream.as_mut() {
            s.schedule(wake)
        }
    }
}
