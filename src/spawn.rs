use {Future, Poll, Executor, Oneshot, Complete, oneshot};

/// A future representing the completion of a spawned subtask.
pub struct Spawn<F: Future> {
    rx: Oneshot<Result<F::Item, F::Error>>,
}

pub fn new<F, E>(fut: F, exec: E) -> Spawn<F>
    where F: Future, E: Executor<SpawnWrap<F>>,
{
    let (tx, mut rx) = oneshot();
    exec.spawn(SpawnWrap { fut: fut, tx: Some(tx) });
    rx.cancel_on_drop = false; // daemonize by default
    Spawn { rx: rx }
}

impl<F: Future> Future for Spawn<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<F::Item, F::Error> {
        match self.rx.poll() {
            Poll::Ok(Ok(x)) => Poll::Ok(x),
            Poll::Ok(Err(e)) => Poll::Err(e),
            Poll::NotReady => Poll::NotReady,

            // TODO: actually propagate the panic info
            Poll::Err(_) => panic!("Propagating panic from spawned subtask"),
        }
    }
}

impl<F: Future> Spawn<F> {
    /// Attempt to cancel the subtask in a best-effort fashion: the subtask will
    /// cease executing at the next "yield" point.
    pub fn cancel(mut self) {
        self.rx.cancel_on_drop = true;
    }
}

/// An internal wrapper used by `spawn` to construct the parent future.
// Wraps a future with transmission along a oneshot, bailing out if the oneshot
// is cancelled.
pub struct SpawnWrap<F: Future> {
    fut: F,
    tx: Option<Complete<Result<F::Item, F::Error>>>,
}

impl<F: Future> Future for SpawnWrap<F> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        if let Poll::Ok(_) = self.tx.as_mut().unwrap().poll_cancel() {
            return Poll::Ok(())
        }

        let res = try_poll!(self.fut.poll());
        self.tx.take().unwrap().complete(res);
        Poll::Ok(())
    }
}
