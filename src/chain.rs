use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::mem;

use slot::Slot;
use util;
use {Future, PollResult};

pub enum Chain<A, B, C> where A: Send + 'static, B: Send + 'static {
    First(A, C),
    Slot(DropSlot<A, B>),
    Moved,
}

pub struct DropSlot<A, B> where A: Send + 'static, B: Send + 'static {
    slot: Arc<ChainSlot<A, B>>,
}

struct ChainSlot<A, B> where A: Send + 'static, B: Send + 'static {
    a_dropped: AtomicBool,
    a: Slot<A>,
    b: Slot<B>,
}

impl<A, B, C> Chain<A, B, C>
    where A: Future,
          B: Future,
          C: Send + 'static,
{
    pub fn new(a: A, c: C) -> Chain<A, B, C> {
        Chain::First(a, c)
    }

    pub fn schedule<G, F>(&mut self, g: G, f: F)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static,
              F: FnOnce(PollResult<A::Item, A::Error>, C)
                        -> PollResult<Result<B::Item, B>, B::Error> + Send + 'static,
    {
        match mem::replace(self, Chain::Moved) {
            Chain::First(mut a, data) => {
                // TODO: we should optimize this allocation. In theory if we
                //       have a chain of combinators (like and_then) then we can
                //       merge a bunch of allocations all into one, and maybe
                //       that's for the best?
                //
                //       The only real reason this exists is so this future can
                //       be used to cancel it, so all we need to do is to
                //       possibly send a "signal" to go cancel the future at
                //       some point.
                let slot = Arc::new(ChainSlot {
                    a_dropped: AtomicBool::new(false),
                    a: Slot::new(None),
                    b: Slot::new(None),
                });
                let slot2 = slot.clone();
                a.schedule(move |r| {
                    // Eagerly drop the first future in case it's holding on to
                    // any resources, it's done and we don't need it any more.
                    //
                    // Note that this should crucially help a recursive chain of
                    // `.and_then(|_| and_then(|_| ..))` from allocating
                    // "infinite memory" because we'll drop the left hand side
                    // ASAP which means that the only memory active is in theory
                    // just for futures in flight.
                    //
                    // TODO: needs a test and probably some assurances that this
                    //       is indeed happening.
                    slot.drop_a();

                    let mut b = match f(r, data) {
                        Ok(Ok(e)) => return g(Ok(e)),
                        Ok(Err(b)) => b,
                        Err(e) => return g(Err(e)),
                    };
                    b.schedule(g);
                    slot.b.try_produce(b).ok().unwrap();
                });
                slot2.a.try_produce(a).ok().unwrap();
                *self = Chain::Slot(DropSlot { slot: slot2 });
            }

            // if we see `Slot` then `schedule` has already been called, and
            // we're not allowed to schedule again after that, so just return a
            // panicked error as this is a contract violation
            Chain::Slot(s) => {
                *self = Chain::Slot(s);
                return g(Err(util::reused()))
            }

            // should be unreachable
            Chain::Moved => panic!(),
        }
    }
}

impl<A, B> ChainSlot<A, B>
    where A: Send + 'static,
          B: Send + 'static
{
    fn drop_a(&self) {
        // Both B's completion and our destructor want to drop A, so we need to
        // coordinate who actually does it.
        if self.a_dropped.swap(true, Ordering::SeqCst) {
            return
        }
        self.a.on_full(|slot| {
            drop(slot.try_consume().ok().unwrap());
        });
    }
}

impl<A, B> Drop for DropSlot<A, B>
    where A: Send + 'static,
          B: Send + 'static
{
    fn drop(&mut self) {
        self.slot.drop_a();
        self.slot.b.on_full(|slot| {
            drop(slot.try_consume().ok().unwrap());
        });
    }
}
