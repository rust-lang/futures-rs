use Async;
use stream::Stream;
use executor;
use task_impl::ThreadNotify;

/// Provides thread-blocking stream iteration.
///
/// See [`blocking`](fn.blocking.html) documentation for more details.
#[derive(Debug)]
pub struct Blocking<T> {
    inner: executor::Spawn<T>,
}

/// Provides thread-blocking stream iteration.
///
/// `blocking` consumes ownership of `stream`, returning a `Blocking` backed by
/// the stream. The `Blocking` value provides a thread-blocking iterator that
/// yields each consecutive value from stream.
///
/// **Iteration will block the current thread**. This means that it should not
/// be performed while in the context of a task executor as this will block the
/// task executor's progress.
pub fn blocking<T: Stream>(stream: T) -> Blocking<T> {
    let inner = executor::spawn(stream);
    Blocking { inner: inner }
}

impl<T> Blocking<T> {
    /// Get a shared reference to the inner stream.
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Consume the `Blocking`, returning its inner stream.
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

impl<T: Stream> Iterator for Blocking<T> {
    type Item = Result<T::Item, T::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        ThreadNotify::with_current(|notify| {
            loop {
                match self.inner.poll_stream_notify(notify, 0) {
                    Ok(Async::Ready(Some(v))) => return Some(Ok(v)),
                    Ok(Async::Ready(None)) => return None,
                    Ok(Async::NotReady) => notify.park(),
                    Err(e) => return Some(Err(e)),
                }
            }
        })
    }
}
