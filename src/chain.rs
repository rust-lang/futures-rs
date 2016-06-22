use std::sync::Arc;
use std::mem;

use executor::{Executor, DEFAULT};
use util;
use {Future, PollResult, Wake};

pub enum Chain<A, B, C> where A: Send + 'static, B: Send + 'static {
    First(A, C),
    Second(B),
    Done,
}

impl<A, B, C> Chain<A, B, C>
    where A: Future,
          B: Future,
          C: Send + 'static,
{
    pub fn new(a: A, c: C) -> Chain<A, B, C> {
        Chain::First(a, c)
    }

    pub fn poll<F>(&mut self, f: F)
                  -> Option<PollResult<B::Item, B::Error>>
        where F: FnOnce(PollResult<A::Item, A::Error>, C)
                        -> PollResult<Result<B::Item, B>, B::Error> + Send + 'static,
    {
        let a_result = match *self {
            Chain::First(ref mut a, _) => {
                match a.poll() {
                    Some(a) => a,
                    None => return None,
                }
            }
            Chain::Second(ref mut b) => return b.poll(),
            Chain::Done => return Some(Err(util::reused())),
        };
        let data = match mem::replace(self, Chain::Done) {
            Chain::First(_, c) => c,
            _ => panic!(),
        };
        match f(a_result, data) {
            Ok(Ok(e)) => Some(Ok(e)),
            Ok(Err(mut b)) => {
                let ret = b.poll();
                *self = Chain::Second(b);
                ret
            }
            Err(e) => Some(Err(e)),
        }
    }

    pub fn schedule(&mut self, wake: Arc<Wake>) {
        match *self {
            Chain::First(ref mut a, _) => a.schedule(wake),
            Chain::Second(ref mut b) => b.schedule(wake),
            Chain::Done => DEFAULT.execute(move || wake.wake()),
        }
    }

    // pub fn schedule<G, F>(&mut self, g: G, f: F)
    //     where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static,
    //           F: FnOnce(PollResult<A::Item, A::Error>, C)
    //                     -> PollResult<Result<B::Item, B>, B::Error> + Send + 'static,
    // {
    //     match mem::replace(self, Chain::Moved) {
    //         Chain::First(mut a, data) => {
    //             // TODO: we should optimize this allocation. In theory if we
    //             //       have a chain of combinators (like and_then) then we can
    //             //       merge a bunch of allocations all into one, and maybe
    //             //       that's for the best?
    //             //
    //             //       The only real reason this exists is so this future can
    //             //       be used to cancel it, so all we need to do is to
    //             //       possibly send a "signal" to go cancel the future at
    //             //       some point.
    //             let slot = Arc::new(ChainSlot {
    //                 a_dropped: AtomicBool::new(false),
    //                 a: Slot::new(None),
    //                 b: Slot::new(None),
    //             });
    //             let slot2 = slot.clone();
    //             a.schedule(move |r| {
    //                 // Eagerly drop the first future in case it's holding on to
    //                 // any resources, it's done and we don't need it any more.
    //                 //
    //                 // Note that this should crucially help a recursive chain of
    //                 // `.and_then(|_| and_then(|_| ..))` from allocating
    //                 // "infinite memory" because we'll drop the left hand side
    //                 // ASAP which means that the only memory active is in theory
    //                 // just for futures in flight.
    //                 //
    //                 // TODO: needs a test and probably some assurances that this
    //                 //       is indeed happening.
    //                 slot.drop_a();
    //
    //                 let mut b = match f(r, data) {
    //                     Ok(Ok(e)) => return DEFAULT.execute(|| g(Ok(e))),
    //                     Ok(Err(b)) => b,
    //                     Err(e) => return DEFAULT.execute(|| g(Err(e))),
    //                 };
    //                 b.schedule(g);
    //                 slot.b.try_produce(b).ok().unwrap();
    //             });
    //             slot2.a.try_produce(a).ok().unwrap();
    //             *self = Chain::Slot(DropSlot { slot: slot2 });
    //         }
    //
    //         // if we see `Slot` then `schedule` has already been called, and
    //         // we're not allowed to schedule again after that, so just return a
    //         // panicked error as this is a contract violation
    //         Chain::Slot(s) => {
    //             *self = Chain::Slot(s);
    //             return DEFAULT.execute(|| g(Err(util::reused())))
    //         }
    //
    //         // should be unreachable
    //         Chain::Moved => panic!(),
    //     }
    // }
}
