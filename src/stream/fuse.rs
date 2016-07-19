use std::sync::Arc;

use {Wake, Tokens, Poll};
use stream::Stream;

/// A stream which "fuse"s a stream once it's terminated.
///
/// Normally streams can behave unpredictably after they've terminated or
/// returned an error, but `Fuse` is always defined to return `None` from `poll`
/// after terination/errors, and afterwards all calls to `schedule` will be
/// ignored.
pub struct Fuse<S> {
    stream: Option<S>,
}

pub fn new<S: Stream>(s: S) -> Fuse<S> {
    Fuse { stream: Some(s) }
}

impl<S: Stream> Stream for Fuse<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens) -> Poll<Option<S::Item>, S::Error> {
        let ret = self.stream.as_mut().map(|s| s.poll(tokens));
        match ret {
            Some(Poll::Ok(None)) => self.stream = None,
            _ => {}
        }
        ret.unwrap_or(Poll::NotReady)
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        if let Some(ref mut stream) = self.stream {
            stream.schedule(wake)
        }
    }
}

impl<S> Fuse<S> {
    // TODO: docs
    #[allow(missing_docs)]
    pub fn is_done(&self) -> bool {
        self.stream.is_none()
    }
}
