use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

use {Future, PollResult, PollError, Wake, Tokens};
use slot::{Slot, Token};
use stream::{Stream, StreamResult};
use util;

pub fn channel<T, E>() -> (Sender<T, E>, Receiver<T, E>)
    where T: Send + 'static,
          E: Send + 'static,
{
    static COUNT: AtomicUsize = ATOMIC_USIZE_INIT;

    let inner = Arc::new(Inner {
        slot: Slot::new(None),
        receiver_gone: AtomicBool::new(false),
        token: COUNT.fetch_add(1, Ordering::SeqCst) + 1,
    });
    let sender = Sender {
        inner: inner.clone(),
    };
    let receiver = Receiver {
        inner: inner,
        on_full_token: None,
        done: false,
    };
    (sender, receiver)
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
    sender: Option<Sender<T, E>>,
    data: Option<Result<T, E>>,
}

pub struct Receiver<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: Arc<Inner<T, E>>,
    on_full_token: Option<Token>,
    done: bool,
}

struct Inner<T, E> {
    slot: Slot<Message<Result<T, E>>>,
    receiver_gone: AtomicBool,
    token: usize,
}

enum Message<T> {
    Data(T),
    Done,
}

pub struct SendError<T, E>(Result<T, E>);

impl<T, E> Stream for Receiver<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self, _tokens: &Tokens) -> Option<StreamResult<T, E>> {
        if self.done {
            return Some(Err(util::reused()))
        }
        // if !tokens.may_contain(&Tokens::from_usize(self.inner.token)) {
        //     return None
        // }

        // TODO: disconnect?
        match self.inner.slot.try_consume() {
            Ok(Message::Data(Ok(e))) => Some(Ok(Some(e))),
            Ok(Message::Data(Err(e))) => Some(Err(PollError::Other(e))),
            Ok(Message::Done) => {
                self.done = true;
                Some(Ok(None))
            }
            Err(..) => None,
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if let Some(token) = self.on_full_token.take() {
            self.inner.slot.cancel(token);
        }

        let token = self.inner.token;
        self.on_full_token = Some(self.inner.slot.on_full(move |_| {
            wake.wake(&Tokens::from_usize(token))
        }));
    }
}

impl<T, E> Drop for Receiver<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        self.inner.receiver_gone.store(true, Ordering::SeqCst);
        if let Some(token) = self.on_full_token.take() {
            self.inner.slot.cancel(token);
        }
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
            sender: Some(self),
            data: Some(t),
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

    fn poll(&mut self, _tokens: &Tokens)
            -> Option<PollResult<Self::Item, Self::Error>> {
        let data = match util::opt2poll(self.data.take()) {
            Ok(data) => data,
            Err(e) => return Some(Err(e)),
        };
        let sender = match util::opt2poll(self.sender.take()) {
            Ok(e) => e,
            Err(e) => return Some(Err(e)),
        };
        match sender.inner.slot.try_produce(Message::Data(data)) {
            Ok(()) => return Some(Ok(sender)),
            Err(e) => {
                self.data = Some(match e.into_inner() {
                    Message::Data(data) => data,
                    Message::Done => panic!(),
                });
                self.sender = Some(sender);
                None
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        match self.sender {
            Some(ref s) => {
                // TODO: don't drop token?
                s.inner.slot.on_empty(move |_slot| {
                    wake.wake(&Tokens::all());
                });
            }
            None => util::done(wake),
        }
    }
}
