use std::sync::Arc;
use std::vec::Vec;
use {Future, Poll, Async, Oneshot, Complete, oneshot};

/// TODO: doc
#[must_use = "futures do nothing unless polled"]
pub struct Multishot<T, E, F>
    where F: Future
{
    completes: Vec<Complete<Result<Arc<T>, Arc<E>>>>,
    original_future: F,
}

/// TODO: doc
pub fn to_multi<F: Future>(future: F) -> Multishot<F::Item, F::Error, F> {
    let multi_shot = Multishot {
        completes: vec![],
        original_future: future,
    };

    multi_shot
}

impl<T, E, F> Multishot<T, E, F>
    where F: Future
{
    /// TODO: doc
    pub fn future(&mut self) -> Oneshot<Result<Arc<T>, Arc<E>>> {
        let (tx, rx) = oneshot();
        self.completes.push(tx);
        rx
    }
}


impl<T, E, F> Future for Multishot<T, E, F>
    where F: Future<Item = T, Error = E>
{
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let result: Poll<T, E> = self.original_future.poll();
        match result {
            Ok(Async::NotReady) => {
                println!("in NotReady");
                Ok(Async::NotReady)
            }
            Ok(Async::Ready(item)) => {
                println!("in Ready");
                let item = Arc::new(item);
                while let Some(c) = self.completes.pop() {
                    c.complete(Ok(item.clone()));
                }
                Ok(Async::Ready(()))
            }
            Err(error) => {
                println!("in Error");
                let error = Arc::new(error);
                while let Some(c) = self.completes.pop() {
                    c.complete(Err(error.clone()));
                }
                Err(())
            }
        }
    }
}
