use {Future, Task, Poll};

/// A future which "fuse"s a future once it's been resolved.
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

    fn poll(&mut self, task: &mut Task) -> Poll<A::Item, A::Error> {
        let ret = self.future.as_mut().map(|f| f.poll(task));
        if ret.as_ref().map(|r| r.is_ready()) == Some(true) {
            self.future = None;
        }
        return ret.unwrap_or(Poll::NotReady)
    }

    fn schedule(&mut self, task: &mut Task) {
        if let Some(ref mut f) = self.future {
            f.schedule(task);
        }
    }
}
