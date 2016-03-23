use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use Future;
use slot::Slot;
use stream::{Stream, PollError};

pub fn channel<T, E>() -> (Sender<T, E>, Receiver<T, E>)
    where T: Send + 'static,
          E: Send + 'static,
{
    let inner = Arc::new(Inner {
        slot: Slot::new(None),
        receiver_gone: AtomicBool::new(false),
    });
    (Sender { inner: inner.clone() }, Receiver { inner: inner })
}

pub struct Sender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: Arc<Inner<T, E>>,
}

pub struct FutureSender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    tx: Sender<T, E>,
    msg: Result<T, E>,
}

pub struct Receiver<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: Arc<Inner<T, E>>,
}

struct Inner<T, E> {
    slot: Slot<Option<Result<T, E>>>,
    receiver_gone: AtomicBool,
}

pub struct SendError<T, E>(Result<T, E>);

impl<T, E> Stream for Receiver<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
        match self.inner.slot.try_consume() {
            Ok(Some(Ok(item))) => Ok(item),
            Ok(Some(Err(item))) => Err(PollError::Other(item)),
            Ok(None) => Err(PollError::Empty),
            Err(..) => Err(PollError::NotReady),
        }
    }

    fn schedule<G>(self, g: G)
        where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
                    Send + 'static
    {
        self.inner.clone().slot.on_full(move |slot| {
            let val = slot.try_consume().ok().unwrap();
            g(val, self);
        })
    }
}

impl<T, E> Drop for Receiver<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        self.inner.receiver_gone.store(true, Ordering::SeqCst);
        self.inner.slot.on_full(|slot| {
            drop(slot.try_consume());
        });
    }
}

impl<T, E> Sender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    pub fn send(self, t: Result<T, E>) -> FutureSender<T, E> {
        FutureSender {
            tx: self,
            msg: t,
        }
    }
}

impl<T, E> Drop for Sender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        self.inner.slot.on_empty(|slot| {
            slot.try_produce(None).ok().unwrap();
        });
    }
}

impl<T, E> Future for FutureSender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = Sender<T, E>;
    type Error = SendError<T, E>;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        let FutureSender { tx, msg } = self;
        if tx.inner.receiver_gone.load(Ordering::SeqCst) {
            return Ok(Err(SendError(msg)))
        }
        match tx.inner.slot.try_produce(Some(msg)) {
            Ok(()) => return Ok(Ok(tx)),
            Err(e) => {
                Err(FutureSender {
                    tx: tx,
                    msg: e.into_inner().unwrap(),
                })
            }
        }
    }

    fn schedule<F>(self, f: F)
        where F: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static,
    {
        let FutureSender { tx, msg } = self;
        tx.inner.clone().slot.on_empty(move |slot| {
            if tx.inner.receiver_gone.load(Ordering::SeqCst) {
                f(Err(SendError(msg)))
            } else {
                slot.try_produce(Some(msg)).ok().unwrap();
                f(Ok(tx))
            }
        });
    }
}
