use std::sync::Arc;
use std::mem;

use slot::Slot;
use util;
use {Future, PollResult, PollError};

pub enum Chain<A, B, C> where B: Send + 'static {
    First(A, Option<C>),
    // Second(B),
    Slot(A, DropSlot<B>),
    Canceled,
}

pub struct DropSlot<B> where B: Send + 'static {
    slot: Arc<Slot<B>>,
}

impl<A, B, C> Chain<A, B, C>
    where A: Future,
          B: Future,
          C: Send + 'static,
{
    pub fn new(a: A, c: C) -> Chain<A, B, C> {
        Chain::First(a, Some(c))
    }

    // pub fn poll<F>(&mut self, f: F) -> Option<PollResult<B::Item, B::Error>>
    //     where F: FnOnce(PollResult<A::Item, A::Error>, C)
    //                     -> PollResult<Result<B::Item, B>, B::Error>
    // {
    //     let mut b = match *self {
    //         Chain::First(ref mut a, ref mut data) => {
    //             let res = match a.poll() {
    //                 Some(res) => res,
    //                 None => return None,
    //             };
    //             let data = util::opt2poll(data.take());
    //             match data.and_then(|data| f(res, data)) {
    //                 Ok(Ok(e)) => return Some(Ok(e)),
    //                 Ok(Err(b)) => b,
    //                 Err(e) => return Some(Err(e)),
    //             }
    //         }
    //         Chain::Second(ref mut b) => return b.poll(),
    //
    //         // TODO: technically this is a contract violation, this is only
    //         //       possible if we have schedule() then cancel() called, and
    //         //       it's not allowed to call poll() after schedule(). Maybe
    //         //       this should return a panic error?
    //         Chain::Canceled => return Some(Err(PollError::Canceled)),
    //
    //         // if we see `Slot` then `schedule` has already been called, and
    //         // we're not allowed to poll after that, so just return a panicked
    //         // error (this is a contract violation)
    //         Chain::Slot(..) => return Some(Err(util::reused())),
    //     };
    //     let ret = b.poll();
    //     *self = Chain::Second(b);
    //     return ret
    // }

    // pub fn cancel(&mut self) {
    //     match *self {
    //         Chain::First(ref mut a, _) => a.cancel(),
    //         // Chain::Second(ref mut b) => return b.cancel(),
    //         Chain::Canceled => return,
    //         Chain::Slot(ref mut a, ref slot) => {
    //             a.cancel();
    //             slot.on_full(|slot| {
    //                 slot.try_consume().ok().unwrap().cancel();
    //             });
    //         }
    //     }
    //     *self = Chain::Canceled;
    // }

    // pub fn await<F>(&mut self, f: F) -> FutureResult<B::Item, B::Error>
    //     where F: FnOnce(PollResult<A::Item, A::Error>, C)
    //                     -> PollResult<Result<B::Item, B>, B::Error>
    // {
    //     let b = match *self {
    //         Chain::First(ref mut a, ref mut data) => {
    //             let data = try!(util::opt2poll(data.take()));
    //             try!(f(a.await().map_err(From::from), data))
    //         }
    //         Chain::Second(ref mut b) => return b.await(),
    //
    //         // TODO: same concern as poll() above
    //         Chain::Canceled => return Err(FutureError::Canceled),
    //
    //         // if we see `Slot` then `schedule` has already been called, and
    //         // we're not allowed to await after that, so just panic as this is a
    //         // contract violation
    //         Chain::Slot(..) => panic!("cannot await a scheduled future"),
    //     };
    //     b.or_else(|mut b| {
    //         let ret = b.await();
    //         *self = Chain::Second(b);
    //         ret
    //     })
    // }

    pub fn schedule<G, F>(&mut self, g: G, f: F)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static,
              F: FnOnce(PollResult<A::Item, A::Error>, C)
                        -> PollResult<Result<B::Item, B>, B::Error> + Send + 'static,
    {
        let (a, slot) = match mem::replace(self, Chain::Canceled) {
            Chain::First(mut a, data) => {
                let data = match util::opt2poll(data) {
                    Ok(data) => data,
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
                    let mut b = match f(r, data) {
                        Ok(Ok(e)) => return g(Ok(e)),
                        Ok(Err(b)) => b,
                        Err(e) => return g(Err(e)),
                    };
                    b.schedule(g);
                    slot.try_produce(b).ok().unwrap();
                });
                (a, slot2)
            }

            // Chain::Second(mut b) => {
            //     b.schedule(g);
            //     *self = Chain::Second(b);
            //     return
            // }

            // TODO: same concern as poll() above
            Chain::Canceled => return g(Err(PollError::Canceled)),

            // if we see `Slot` then `schedule` has already been called, and
            // we're not allowed to schedule again after that, so just return a
            // panicked error as this is a contract violation
            Chain::Slot(a, s) => {
                *self = Chain::Slot(a, s);
                return g(Err(util::reused()))
            }
        };

        *self = Chain::Slot(a, DropSlot { slot: slot });
    }
}

impl<B> Drop for DropSlot<B>
    where B: Send + 'static
{
    fn drop(&mut self) {
        self.slot.on_full(|slot| {
            drop(slot.try_consume().ok().unwrap());
        });
    }
}
