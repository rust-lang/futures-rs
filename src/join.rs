use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use {PollResult, Callback, Future, PollError};
use cell;
use util;

pub struct Join<A, B> where A: Future, B: Future<Error=A::Error> {
    state: State<A, B>,
}

pub fn new<A, B>(a: A, b: B) -> Join<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    Join {
        state: State::Start(a, b),
    }
}

impl<A, B> Future for Join<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    type Item = (A::Item, B::Item);
    type Error = A::Error;

    // fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
    //     let (a, b) = match self.state {
    //         State::Start(ref mut a, ref mut b) => (a, b),
    //         State::Canceled => return Some(Err(PollError::Canceled)),
    //         State::Scheduled(_) => return Some(Err(util::reused())),
    //     };
    //     let a_res = self.a_res.take().map(Ok).or_else(|| a.poll());
    //     let b_res = self.b_res.take().map(Ok).or_else(|| b.poll());
    //     match (a_res, b_res) {
    //         (Some(Err(e)), _) => {
    //             b.cancel();
    //             Some(Err(e))
    //         }
    //         (_, Some(Err(e))) => {
    //             a.cancel();
    //             Some(Err(e))
    //         }
    //         (None, Some(Ok(b))) => {
    //             self.b_res = Some(b);
    //             None
    //         }
    //         (Some(Ok(a)), None) => {
    //             self.a_res = Some(a);
    //             None
    //         }
    //         (None, None) => None,
    //         (Some(Ok(a)), Some(Ok(b))) => Some(Ok((a, b))),
    //     }
    // }

    // fn await(&mut self) -> FutureResult<(A::Item, B::Item), A::Error> {
    //     // TODO: shouldn't wait for a first, b may finish first with an error
    //     let a = self.a_res.take().map(Ok).unwrap_or_else(|| self.a.await());
    //     let a = match a {
    //         Ok(a) => a,
    //         Err(e) => {
    //             self.b.cancel();
    //             return Err(e)
    //         }
    //     };
    //     let b = self.b_res.take().map(Ok).unwrap_or_else(|| self.b.await());
    //     b.map(|b| (a, b))
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
    //             // We only actually do the cancellation if:
    //             //
    //             // * An error hasn't happened. If one has happened that whomever
    //             //   set that flag is responsible for cancellation.
    //             // * We're not done yet, in this case cancellation isn't
    //             //   necessary.
    //             if old & (A_ERR | B_ERR) == 0 &&
    //                old & (A_OK | B_OK) != A_OK | B_OK {
    //                 state.cancel(old);
    //             }
    //         }
    //         State::Canceled => {}
    //     }
    //     self.state = State::Canceled;
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
    {
        // TODO: pretty unfortunate we gotta box this up
        self.schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        let (mut a, mut b) = match mem::replace(&mut self.state, State::Canceled) {
            State::Start(a, b) => (a, b),
            State::Canceled => return cb.call(Err(PollError::Canceled)),
            State::Scheduled(s) => {
                self.state = State::Scheduled(s);
                return cb.call(Err(util::reused()))
            }
        };

        // TODO: optimize the case that both futures are immediately done.
        let data1 = Arc::new(Scheduled {
            futures: cell::AtomicCell::new(None),
            a_val: cell::AtomicCell::new(None),
            b_val: cell::AtomicCell::new(None),
            state: AtomicUsize::new(0),
            cb: cell::AtomicCell::new(Some(cb)),
        });
        let data2 = data1.clone();
        let data3 = data2.clone();

        a.schedule(move |result| data1.finish(&data1.a_val, result, A_OK));
        b.schedule(move |result| data2.finish(&data2.b_val, result, B_OK));
        *data3.futures.borrow().expect("[j] futures locked") = Some((a, b));

        // Tell the state that we've now placed the futures so they can be
        // canceled. If, however, an error already happened then we need to
        // cancel them ourselves.
        let old = data3.state.fetch_or(SET, Ordering::SeqCst);
        if old & (A_ERR | B_ERR) != 0 {
            data3.cancel();
        }

        self.state = State::Scheduled(data3);
    }
}

enum State<A, B> where A: Future, B: Future<Error=A::Error> {
    Start(A, B),
    Scheduled(Arc<Scheduled<A, B>>),
    Canceled,
}

const A_OK: usize = 1 << 0;
const A_ERR: usize = 1 << 1;
const B_OK: usize = 1 << 2;
const B_ERR: usize = 1 << 3;
const SET: usize = 1 << 4;

struct Scheduled<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    futures: cell::AtomicCell<Option<(A, B)>>,
    a_val: cell::AtomicCell<Option<A::Item>>,
    b_val: cell::AtomicCell<Option<B::Item>>,
    state: AtomicUsize,
    cb: cell::AtomicCell<Option<Box<Callback<(A::Item, B::Item), A::Error>>>>,
}

impl<A, B> Scheduled<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    fn finish<T>(&self,
                 slot: &cell::AtomicCell<Option<T>>,
                 val: PollResult<T, A::Error>,
                 flag: usize) {
        let err = match val {
            Ok(t) => {
                let mut slot = slot.borrow().expect("[j] cannot lock own slot");
                assert!(slot.is_none());
                *slot = Some(t);
                None
            }
            Err(e) => Some(e),
        };

        let (okflag, errflag) = (flag, flag << 1);
        let newflag = if err.is_some() {errflag} else {okflag};
        let old = self.state.fetch_or(newflag, Ordering::SeqCst);
        assert!(old & okflag == 0);
        assert!(old & errflag == 0);

        let otherok = if flag == A_OK {B_OK} else {A_OK};
        let othererr = otherok << 1;

        if old & (othererr | otherok) == 0 {
            // if the other side hasn't finished, then we only go through below
            // if we hit an error, if we finished ok then we bail out
            if newflag == okflag {
                return
            }
        } else if old & othererr != 0 {
            // if the other side hit an error, they're doing cleanup
            return
        }

        // If we're here, then we're in one of two situations:
        //
        // * The other side hasn't done anything and we hit an error
        // * The other side finished ok and we either hit an error or finished
        //   ok
        //
        // In both cases we're responsible for cleaning up, so all the takes()
        // here are assertions.

        let cb = self.cb.borrow().expect("[j] done but cb is locked")
                        .take().expect("[j] done done but cb not here");
        if let Some(e) = err {
            // If the futures have made their way over to us, then we cancel
            // them both here. Otherwise the thread putting the futures into
            // place will see the error of its ways and cancel them for us.
            if old & SET != 0 {
                self.cancel();
            }
            cb.call(Err(e))
        } else {
            let a = self.a_val.borrow().expect("[j] done, but a locked")
                              .take().expect("[j] done but a not here");
            let b = self.b_val.borrow().expect("[j] done, but b locked")
                              .take().expect("[j] done but b not here");
            cb.call(Ok((a, b)))
        }
    }

    fn cancel(&self) {
        let pair = self.futures.borrow().expect("[j] futures locked in cancel")
                               .take().expect("[j] cancel but futures not here");
        drop(pair)
    }
}

impl<A, B> Drop for Join<A, B> where A: Future, B: Future<Error=A::Error> {
    fn drop(&mut self) {
        if let State::Scheduled(ref state) = self.state {
            // Unset the `SET` flag so we can attempt to "lock" the futures'
            // memory to get acquired.
            let old = state.state.fetch_xor(SET, Ordering::SeqCst);
            assert!(old & SET != 0);

            // We only actually do the cancellation if:
            //
            // * An error hasn't happened. If one has happened that whomever
            //   set that flag is responsible for cancellation.
            // * We're not done yet, in this case cancellation isn't
            //   necessary.
            if old & (A_ERR | B_ERR) == 0 &&
               old & (A_OK | B_OK) != A_OK | B_OK {
                state.cancel();
            }
        }
    }
}
