use std::sync::Arc;
use std::mem;

use {PollResult, Wake, Future};
use executor::{Executor, DEFAULT};
use util;

/// Future for the `join` combinator, waiting for two futures to complete.
///
/// This is created by this `Future::join` method.
pub struct Join<A, B> where A: Future, B: Future<Error=A::Error> {
    a: MaybeDone<A>,
    b: MaybeDone<B>,
}

enum MaybeDone<A: Future> {
    NotYet(A),
    Done(A::Item),
    Gone,
}

pub fn new<A, B>(a: A, b: B) -> Join<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    Join {
        a: MaybeDone::NotYet(a),
        b: MaybeDone::NotYet(b),
    }
}

impl<A, B> Future for Join<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    type Item = (A::Item, B::Item);
    type Error = A::Error;

    fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
        match (self.a.poll(), self.b.poll()) {
            (Ok(true), Ok(true)) => Some(Ok((self.a.take(), self.b.take()))),
            (Err(e), _) |
            (_, Err(e)) => {
                self.a = MaybeDone::Gone;
                self.b = MaybeDone::Gone;
                Some(Err(e))
            }
            (Ok(_), Ok(_)) => None,
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        match (&mut self.a, &mut self.b) {
            // need to wait for both
            (&mut MaybeDone::NotYet(ref mut a),
             &mut MaybeDone::NotYet(ref mut b)) => {
                a.schedule(wake.clone());
                b.schedule(wake);
            }

            // Only need to wait for one
            (&mut MaybeDone::NotYet(ref mut a), _) => a.schedule(wake),
            (_, &mut MaybeDone::NotYet(ref mut b)) => b.schedule(wake),

            // We're "ready" as we'll return a panicked response
            (&mut MaybeDone::Gone, _) |
            (_, &mut MaybeDone::Gone) => DEFAULT.execute(move || wake.wake()),

            // Shouldn't be possible, can't get into this state
            (&mut MaybeDone::Done(_), &mut MaybeDone::Done(_)) => panic!(),
        }
    }
}

impl<A: Future> MaybeDone<A> {
    fn poll(&mut self) -> PollResult<bool, A::Error> {
        let res = match *self {
            MaybeDone::NotYet(ref mut a) => a.poll(),
            MaybeDone::Done(_) => return Ok(true),
            MaybeDone::Gone => return Err(util::reused()),
        };
        match res {
            Some(res) => {
                *self = MaybeDone::Done(try!(res));
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn take(&mut self) -> A::Item {
        match mem::replace(self, MaybeDone::Gone) {
            MaybeDone::Done(a) => a,
            _ => panic!(),
        }
    }
}
