use std::sync::Arc;
use std::mem;

use slot::Slot;
use util;
use {Future, PollResult};

pub enum Chain<A, B, C> where B: Send + 'static {
    First(A, C),
    Slot(A, DropSlot<B>),
    Moved,
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
        Chain::First(a, c)
    }

    pub fn schedule<G, F>(&mut self, g: G, f: F)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static,
              F: FnOnce(PollResult<A::Item, A::Error>, C)
                        -> PollResult<Result<B::Item, B>, B::Error> + Send + 'static,
    {
        let (a, slot) = match mem::replace(self, Chain::Moved) {
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

            // if we see `Slot` then `schedule` has already been called, and
            // we're not allowed to schedule again after that, so just return a
            // panicked error as this is a contract violation
            Chain::Slot(a, s) => {
                *self = Chain::Slot(a, s);
                return g(Err(util::reused()))
            }

            // should be unreachable
            Chain::Moved => panic!(),
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
