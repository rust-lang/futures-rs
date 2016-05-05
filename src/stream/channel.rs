use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use {Future, PollResult, PollError, Callback};
use slot::{Slot, Token};
use stream::{Stream, StreamResult};
use util;

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
    state: _FutureSender<T, E>,
}

enum _FutureSender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Start(Sender<T, E>, Result<T, E>),
    Canceled,
    Used,
    Scheduled(Arc<Inner<T, E>>, Token),
}

pub struct Receiver<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: Arc<Inner<T, E>>,
}

struct Inner<T, E> {
    slot: Slot<Message<Result<T, E>>>,
    receiver_gone: AtomicBool,
}

enum Message<T> {
    Data(T),
    Done,
    // Canceled,
}

pub struct SendError<T, E>(Result<T, E>);

impl<T, E> Stream for Receiver<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<StreamResult<Self::Item, Self::Error>> {
        match self.inner.slot.try_consume() {
            Ok(Message::Data(Ok(e))) => Some(Ok(Some(e))),
            Ok(Message::Data(Err(e))) => Some(Err(PollError::Other(e))),
            Ok(Message::Done) => Some(Ok(None)),
            // Ok(Message::Canceled) => Some(Err(PollError::Canceled)),
            Err(..) => None,
        }
    }

    fn cancel(&mut self) {
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(StreamResult<Self::Item, Self::Error>) + Send + 'static
    {
        self.inner.slot.on_full(|slot| {
            g(match slot.try_consume() {
                Ok(Message::Data(Ok(e))) => Ok(Some(e)),
                Ok(Message::Data(Err(e))) => Err(PollError::Other(e)),
                Ok(Message::Done) => Ok(None),
                // Ok(Message::Canceled) => Err(PollError::Canceled),
                Err(..) => Err(PollError::Canceled),
            })
        });
    }

    fn schedule_boxed(&mut self,
                      g: Box<Callback<Option<Self::Item>, Self::Error>>) {
        self.schedule(|f| g.call(f))
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
            state: _FutureSender::Start(self, t),
        }
    }
}

impl<T, E> Drop for Sender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        self.inner.slot.on_empty(|slot| {
            slot.try_produce(Message::Done).ok().unwrap();
        });
    }
}

impl<T, E> Future for FutureSender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = Sender<T, E>;
    type Error = SendError<T, E>;

    // fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
    //     match mem::replace(&mut self.state, _FutureSender::Used) {
    //         _FutureSender::Start(tx, msg) => {
    //             if tx.inner.receiver_gone.load(Ordering::SeqCst) {
    //                 return Some(Err(PollError::Other(SendError(msg))))
    //             }
    //             match tx.inner.slot.try_produce(Message::Data(msg)) {
    //                 Ok(()) => Some(Ok(tx)),
    //                 Err(e) => {
    //                     let msg = match e.into_inner() {
    //                         Message::Data(d) => d,
    //                         _ => panic!(),
    //                     };
    //                     self.state = _FutureSender::Start(tx, msg);
    //                     None
    //                 }
    //             }
    //         }
    //         _FutureSender::Canceled => Some(Err(PollError::Canceled)),
    //         _FutureSender::Used => Some(Err(util::reused())),
    //         _FutureSender::Scheduled(s, token) => {
    //             self.state = _FutureSender::Scheduled(s, token);
    //             Some(Err(util::reused()))
    //         }
    //     }
    // }

    // TODO: move this to Drop
    // fn cancel(&mut self) {
    //     match mem::replace(&mut self.state, _FutureSender::Canceled) {
    //         _FutureSender::Start(..) => {}
    //         _FutureSender::Canceled => {}
    //         _FutureSender::Scheduled(s, token) => {
    //             s.slot.cancel(token);
    //         }
    //         _FutureSender::Used => self.state = _FutureSender::Used,
    //     }
    // }

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static,
    {
        let (tx, msg) = match mem::replace(&mut self.state, _FutureSender::Used) {
            _FutureSender::Start(tx, msg) => (tx, msg),
            _FutureSender::Canceled => return f(Err(PollError::Canceled)),
            _FutureSender::Used => return f(Err(util::reused())),
            _FutureSender::Scheduled(s, token) => {
                self.state = _FutureSender::Scheduled(s, token);
                return f(Err(util::reused()))
            }
        };
        let arc = tx.inner.clone();
        let token = arc.slot.on_empty(|slot| {
            if tx.inner.receiver_gone.load(Ordering::SeqCst) {
                return f(Err(PollError::Other(SendError(msg))))
            }
            match slot.try_produce(Message::Data(msg)) {
                Ok(()) => f(Ok(tx)),

                // we were canceled so finish immediately
                Err(..) => f(Err(PollError::Canceled)),
            }
        });
        self.state = _FutureSender::Scheduled(arc, token);
    }

    fn schedule_boxed(&mut self, f: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| f.call(r))
    }
}
