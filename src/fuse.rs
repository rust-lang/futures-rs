use std::sync::Arc;

use {Future, Wake, Tokens};

/// A future which "fuse"s an future once it's been resolved.
///
/// Normally futures can behave unpredictable once they're used after a future
/// has been resolved, but `Fuse` is always defined to return `None` from `poll`
/// after it has succeeded, and after it has succeeded all future calls to
/// `schedule` will be ignored.
pub struct Fuse<A> {
    future: Option<A>,
}

pub fn new<A: Future>(f: A) -> Fuse<A> {
    Fuse {
        future: Some(f),
    }
}

impl<A: Future> Future for Fuse<A> {
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<Result<A::Item, A::Error>> {
        let ret = self.future.as_mut().and_then(|f| f.poll(tokens));
        if ret.is_some() {
            self.future = None;
        }
        return ret
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if let Some(ref mut f) = self.future {
            f.schedule(wake);
        }
    }
}
