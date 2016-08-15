use {Future, Poll};
use task::Task;

pub fn forget<T: Future + Send>(t: T) {
    let thunk = ThunkFuture { inner: t };
    Task::new().run(Box::new(thunk))
}

// FIXME(rust-lang/rust#34416) should just be able to use map/map_err, but that
//                             causes trans to go haywire.
struct ThunkFuture<T: Future> {
    inner: T,
}

impl<T: Future> Future for ThunkFuture<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        self.inner.poll().map(|_| ()).map_err(|_| ())
    }
}
