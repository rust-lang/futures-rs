use std::sync::Arc;

use {Future, Wake, Tokens};

/// A future which "fuse"s an future once it's been resolved.
///
/// Normally futures can behave unpredictable once they're used after a future
/// has been resolved, but `Fuse` is always defined to return `None` from `poll`
/// after it has succeeded, and after it has succeeded all future calls to
/// `schedule` will be ignored.
pub struct Fuse<A> {
    future: A,
    done: bool,
}

pub fn new<A: Future>(f: A) -> Fuse<A> {
    Fuse {
        future: f,
        done: false,
    }
}

impl<A: Future> Future for Fuse<A> {
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<Result<A::Item, A::Error>> {
        if self.done {
            None
        } else {
            let res = self.future.poll(tokens);
            self.done = res.is_some();
            return res
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if !self.done {
            self.future.schedule(wake);
        }
    }
}
