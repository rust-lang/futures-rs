use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use {PollResult, Callback, Future, PollError};
use cell;
use util;

pub struct Select<A, B> where A: Future, B: Future<Item=A::Item, Error=A::Error> {
    state: State<A, B>,
}

pub fn new<A, B>(a: A, b: B) -> Select<A, B>
    where A: Future,
          B: Future<Item=A::Item, Error=A::Error>
{
    Select {
        state: State::Start(a, b),
    }
}

impl<A, B> Future for Select<A, B>
    where A: Future,
          B: Future<Item=A::Item, Error=A::Error>,
{
    type Item = A::Item;
    type Error = A::Error;

    // fn poll(&mut self) -> Option<PollResult<A::Item, A::Error>> {
    //     let (a, b) = match self.state {
    //         State::Start(ref mut a, ref mut b) => (a, b),
    //         State::Canceled => return Some(Err(PollError::Canceled)),
    //         State::Scheduled(_) => return Some(Err(util::reused())),
    //     };
    //     if let Some(e) = a.poll() {
    //         b.cancel();
    //         Some(e)
    //     } else if let Some(e) = b.poll() {
    //         a.cancel();
    //         Some(e)
    //     } else {
    //         None
    //     }
    // }

    // fn cancel(&mut self) {
    //     match self.state {
    //         State::Start(ref mut a, ref mut b) => {
    //             a.cancel();
    //             b.cancel();
    //         }
    //         State::Scheduled(ref state) => {
    //             // Unset the `SET` flag so we can attempt to "lock" the futures'
    //             // memory to get acquired.
    //             let old = state.state.fetch_xor(SET, Ordering::SeqCst);
    //             assert!(old & SET != 0);
    //
    //             // We only actually do the cancellation if nothing has actually
    //             // happened yet. If either future has completed then it will
    //             // take care of the cancellation for us.
    //             if old & !SET == 0 {
    //                 state.cancel(old);
    //             }
    //         }
    //         State::Canceled => {}
    //     }
    //     self.state = State::Canceled;
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<A::Item, A::Error>) + Send + 'static
    {
        // TODO: pretty unfortunate we gotta box this up
        self.schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<A::Item, A::Error>>) {
        let (mut a, mut b) = match mem::replace(&mut self.state, State::Canceled) {
            State::Start(a, b) => (a, b),
            State::Canceled => return cb.call(Err(PollError::Canceled)),
            State::Scheduled(s) => {
                self.state = State::Scheduled(s);
                return cb.call(Err(util::reused()))
            }
        };
        let data1 = Arc::new(Scheduled {
            futures: cell::AtomicCell::new(None),
            state: AtomicUsize::new(0),
            cb: cell::AtomicCell::new(Some(cb)),
        });
        let data2 = data1.clone();
        let data3 = data2.clone();

        a.schedule(move |result| data1.finish(result, A_OK));
        b.schedule(move |result| data2.finish(result, B_OK));
        *data3.futures.borrow().expect("[s] futures locked") = Some((a, b));

        // Tell the state that we've now placed the futures so they can be
        // canceled. If, however, an error already happened or someone already
        // finished then we need to cancel them ourselves.
        let old = data3.state.fetch_or(SET, Ordering::SeqCst);
        assert!(old & SET == 0);
        if old != 0 {
            data3.cancel();
        }

        self.state = State::Scheduled(data3);
    }
}

enum State<A, B> where A: Future, B: Future {
    Start(A, B),
    Scheduled(Arc<Scheduled<A, B>>),
    Canceled,
}

const A_OK: usize = 1 << 0;
#[allow(dead_code)]
const A_ERR: usize = 1 << 1;
const B_OK: usize = 1 << 2;
#[allow(dead_code)]
const B_ERR: usize = 1 << 3;
const SET: usize = 1 << 4;

struct Scheduled<A, B>
    where A: Future,
          B: Future,
{
    futures: cell::AtomicCell<Option<(A, B)>>,
    state: AtomicUsize,
    cb: cell::AtomicCell<Option<Box<Callback<A::Item, A::Error>>>>,
}

impl<A, B> Scheduled<A, B>
    where A: Future,
          B: Future<Item=A::Item, Error=A::Error>,
{
    fn finish(&self, val: PollResult<A::Item, A::Error>, flag: usize) {
        let (okflag, errflag) = (flag, flag << 1);
        let newflag = if val.is_err() {errflag} else {okflag};
        let old = self.state.fetch_or(newflag, Ordering::SeqCst);
        assert!(old & okflag == 0);
        assert!(old & errflag == 0);

        let otherok = if flag == A_OK {B_OK} else {A_OK};
        let othererr = otherok << 1;

        // if the other side finished before we did then we just drop our result
        // on the ground and let them take care of everything.
        if old & (othererr | otherok) != 0 {
            return
        }

        let cb = self.cb.borrow().expect("[s] done but cb is locked")
                        .take().expect("[s] done done but cb not here");
        // If the futures have made their way over to us, then we cancel
        // them both here. Otherwise the thread putting the futures into
        // place will see the error of its ways and cancel them for us.
        if old & SET != 0 {
            self.cancel();
        }
        cb.call(val);
    }

    fn cancel(&self) {
        let pair = self.futures.borrow().expect("[s] futures locked in cancel")
                               .take().expect("[s] cancel but futures not here");
        drop(pair)
    }
}

impl<A, B> Drop for Select<A, B>
    where A: Future,
          B: Future<Item=A::Item, Error=A::Error>
{
    fn drop(&mut self) {
        if let State::Scheduled(ref state) = self.state {
            // Unset the `SET` flag so we can attempt to "lock" the futures'
            // memory to get acquired.
            let old = state.state.fetch_xor(SET, Ordering::SeqCst);
            assert!(old & SET != 0);

            // We only actually do the cancellation if nothing has actually
            // happened yet. If either future has completed then it will
            // take care of the cancellation for us.
            if old & !SET == 0 {
                state.cancel();
            }
        }
    }
}
