use {Future, Poll, Task};
use util::Collapsed;

pub fn forget<T: Future>(t: T) {
    let thunk = ThunkFuture { inner: Collapsed::Start(t) }.boxed();
    Task::new().run(thunk)
}

// FIXME(rust-lang/rust#34416) should just be able to use map/map_err, but that
//                             causes trans to go haywire.
struct ThunkFuture<T: Future> {
    inner: Collapsed<T>,
}

impl<T: Future> Future for ThunkFuture<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self, task: &mut Task) -> Poll<(), ()> {
        self.inner.poll(task).map(|_| ()).map_err(|_| ())
    }

    fn schedule(&mut self, task: &mut Task) {
        self.inner.schedule(task)
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=(), Error=()>>> {
        self.inner.collapse();
        None
    }
}
