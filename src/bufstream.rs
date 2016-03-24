use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::iter;

use Future;
use buf::AtomicBuf;
use cell::AtomicCell;
use channel::{channel, Sender, Receiver};

const LOCKED: usize = 1 << 31;

pub fn bufstream<T, E>(n: usize) -> (BufSenders<T, E>, Receiver<T, E>)
    where T: Send + 'static,
          E: Send + 'static,
{
    // TODO: verify `n` is less than LOCKED
    let (tx, rx) = channel();
    let inner = Arc::new(Inner::new(n, tx));
    let tx = iter::repeat(inner.clone()).take(n);
    (BufSenders { inner: tx }, rx)
}

pub struct BufSenders<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: iter::Take<iter::Repeat<Arc<Inner<T, E>>>>,
}

pub struct BufSender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: Arc<Inner<T, E>>,
}

struct Inner<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    buf: AtomicBuf<Result<T, E>>,
    sender: AtomicCell<Option<Sender<T, E>>>,
    state: AtomicUsize,
}

impl<T, E> Iterator for BufSenders<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = BufSender<T, E>;

    fn next(&mut self) -> Option<BufSender<T, E>> {
        self.inner.next().map(|tx| BufSender { inner: tx })
    }
}

impl<T, E> BufSender<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    pub fn send(self, t: Result<T, E>) {
        self.inner.buf.push(t);
        self.inner.state.fetch_add(1, Ordering::SeqCst);
        Inner::maybe_send(self.inner);
    }
}

impl<T, E> Inner<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn new(amt: usize, sender: Sender<T, E>) -> Inner<T, E> {
        Inner {
            buf: AtomicBuf::new(amt),
            sender: AtomicCell::new(Some(sender)),
            state: AtomicUsize::new(0),
        }
    }

    fn maybe_send(me: Arc<Self>) {
        let mut state = me.state.load(Ordering::SeqCst);
        loop {
            if state & LOCKED != 0 || state == 0 {
                return
            }
            let old = me.state.compare_and_swap(state, state | LOCKED,
                                                Ordering::SeqCst);
            if old == state {
                break
            }
            state = old;
        }
        let sender = me.sender.borrow().unwrap().take().unwrap();
        Inner::do_send(me, sender);
    }

    fn do_send(me: Arc<Self>, sender: Sender<T, E>) {
        match me.buf.pop() {
            Some(t) => {
                let prev = me.state.fetch_sub(1, Ordering::SeqCst);
                assert!(prev & LOCKED == LOCKED);
                assert!((prev & !LOCKED) > 0);

                sender.send(t).schedule(move |res| {
                    match res {
                        Ok(sender) => Inner::do_send(me, sender),

                        // our receiver has disappeared, just drop our data on
                        // the ground and just let us wallow away in the ether.
                        Err(..) => {}
                    }
                });
            }
            None => {
                *me.sender.borrow().unwrap() = Some(sender);
                me.state.fetch_xor(LOCKED, Ordering::SeqCst);
                Inner::maybe_send(me);
            }
        }
    }
}
