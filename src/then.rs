use std::sync::Arc;
use std::mem;

use {Future, IntoFuture, PollError, Callback, PollResult, FutureResult};
use FutureError;
use util;
use slot::Slot;

pub struct Then<A, B, F> {
    state: _Then<A, B, F>,
}

enum _Then<A, B, F> {
    First(A, Option<F>),
    Second(B),
    Slot(A, Arc<Slot<B>>),
    Canceled,
}

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F> {
    Then {
        state: _Then::First(future, Some(f)),
    }
}

fn then<A, B, C, D, F>(a: PollResult<A, B>, f: F) -> PollResult<C, D>
    where F: FnOnce(Result<A, B>) -> C + Send + 'static,
          A: Send + 'static,
          B: Send + 'static,
{
    match a {
        Ok(e) => util::recover(|| f(Ok(e))),
        Err(PollError::Other(e)) => util::recover(|| f(Err(e))),
        Err(PollError::Panicked(e)) => Err(PollError::Panicked(e)),
        Err(PollError::Canceled) => Err(PollError::Canceled),
    }
}

impl<A, B, F> Future for Then<A, B::Future, F>
    where A: Future,
          B: IntoFuture,
          F: FnOnce(Result<A::Item, A::Error>) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self) -> Option<PollResult<B::Item, B::Error>> {
        let mut b = match self.state {
            _Then::First(ref mut a, ref mut f) => {
                let res = match a.poll() {
                    Some(res) => res,
                    None => return None,
                };
                let f = util::opt2poll(f.take());
                match f.and_then(|f| then(res, f)) {
                    Ok(b) => b.into_future(),
                    Err(e) => return Some(Err(e)),
                }
            }
            _Then::Second(ref mut b) => return b.poll(),

            // TODO: technically this is a contract violation, this is only
            //       possible if we have schedule() then cancel() called, and
            //       it's not allowed to call poll() after schedule(). Maybe
            //       this should return a panic error?
            _Then::Canceled => return Some(Err(PollError::Canceled)),

            // if we see `Slot` then `schedule` has already been called, and
            // we're not allowed to poll after that, so just return a panicked
            // error (this is a contract violation)
            _Then::Slot(..) => return Some(Err(util::reused())),
        };
        let ret = b.poll();
        self.state = _Then::Second(b);
        return ret
    }

    fn cancel(&mut self) {
        match self.state {
            _Then::First(ref mut a, _) => return a.cancel(),
            _Then::Second(ref mut b) => return b.cancel(),
            _Then::Canceled => return,
            _Then::Slot(ref mut a, ref slot) => {
                a.cancel();
                slot.on_full(|slot| {
                    slot.try_consume().ok().unwrap().cancel();
                });
            }
        }
        self.state = _Then::Canceled;
    }

    fn await(&mut self) -> FutureResult<B::Item, B::Error> {
        match self.state {
            _Then::First(ref mut a, ref mut f) => {
                let f = try!(util::opt2poll(f.take()));
                match a.await() {
                    Ok(e) => f(Ok(e)).into_future().await(),
                    Err(FutureError::Other(e)) => f(Err(e)).into_future().await(),
                    Err(FutureError::Canceled) => Err(FutureError::Canceled)
                }
            }
            _Then::Second(ref mut b) => b.await(),

            // TODO: same concern as poll() above
            _Then::Canceled => Err(FutureError::Canceled),

            // if we see `Slot` then `schedule` has already been called, and
            // we're not allowed to await after that, so just panic as this is a
            // contract violation
            _Then::Slot(..) => panic!("cannot await a scheduled future"),
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        let (a, slot) = match mem::replace(&mut self.state, _Then::Canceled) {
            _Then::First(mut a, mut f) => {
                let f = match util::opt2poll(f.take()) {
                    Ok(f) => f,
                    Err(e) => return g(Err(e)),
                };

                // TODO: we should optimize this allocation. In theory if we
                //       have a chain of combinators (like and_then) then we can
                //       merge a bunch of allocations all into one, and maybe
                //       that's for the best?
                //
                //       The only real reason this exists is so this future can
                //       be used to cancel it, so all we need to do is to
                //       possibly send a "signal" to go cancel the future at
                //       some point.
                let slot = Arc::new(Slot::new(None));
                let slot2 = slot.clone();
                a.schedule(move |r| {
                    let mut b = match then(r, f) {
                        Ok(b) => b.into_future(),
                        Err(e) => return g(Err(e)),
                    };
                    b.schedule(g);
                    slot.try_produce(b).ok().unwrap();
                });
                (a, slot2)
            }

            _Then::Second(mut b) => {
                b.schedule(g);
                self.state = _Then::Second(b);
                return
            }

            // TODO: same concern as poll() above
            _Then::Canceled => return g(Err(PollError::Canceled)),

            // if we see `Slot` then `schedule` has already been called, and
            // we're not allowed to schedule again after that, so just return a
            // panicked error as this is a contract violation
            _Then::Slot(a, s) => {
                self.state = _Then::Slot(a, s);
                return g(Err(util::reused()))
            }
        };

        self.state = _Then::Slot(a, slot);
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        // TODO: wut? UFCS?
        Future::schedule(self, |r| cb.call(r))
    }
}
