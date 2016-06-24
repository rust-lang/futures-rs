use std::sync::Arc;
use std::mem;

use {PollResult, Wake, Future, Tokens};
use util::{self, Collapsed};

/// Future for the `join` combinator, waiting for two futures to complete.
///
/// This is created by this `Future::join` method.
pub struct Join<A, B> where A: Future, B: Future<Error=A::Error> {
    a: MaybeDone<A>,
    b: MaybeDone<B>,
}

enum MaybeDone<A: Future> {
    NotYet(Collapsed<A>, Tokens),
    Done(A::Item),
    Gone,
}

pub fn new<A, B>(a: A, b: B) -> Join<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    let a = Collapsed::Start(a);
    let b = Collapsed::Start(b);
    Join {
        a: MaybeDone::NotYet(a, Tokens::all()),
        b: MaybeDone::NotYet(b, Tokens::all()),
    }
}

impl<A, B> Future for Join<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    type Item = (A::Item, B::Item);
    type Error = A::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<PollResult<Self::Item, Self::Error>> {
        match (self.a.poll(tokens), self.b.poll(tokens)) {
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

    fn schedule(&mut self, wake: Arc<Wake>) -> Tokens {
        match (&mut self.a, &mut self.b) {
            // need to wait for both
            (&mut MaybeDone::NotYet(ref mut a, ref mut a_tokens),
             &mut MaybeDone::NotYet(ref mut b, ref mut b_tokens)) => {
                *a_tokens = a.schedule(wake.clone());
                *b_tokens = b.schedule(wake);
                &*a_tokens | &*b_tokens
            }

            // Only need to wait for one
            (&mut MaybeDone::NotYet(ref mut a, ref mut tokens), _) => {
                *tokens = a.schedule(wake);
                tokens.clone()
            }
            (_, &mut MaybeDone::NotYet(ref mut b, ref mut tokens)) => {
                *tokens = b.schedule(wake);
                tokens.clone()
            }

            // We're "ready" as we'll return a panicked response
            (&mut MaybeDone::Gone, _) |
            (_, &mut MaybeDone::Gone) => util::done(wake),

            // Shouldn't be possible, can't get into this state
            (&mut MaybeDone::Done(_), &mut MaybeDone::Done(_)) => panic!(),
        }
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        self.a.collapse();
        self.b.collapse();
        None
    }
}

impl<A: Future> MaybeDone<A> {
    fn poll(&mut self, tokens: &Tokens) -> PollResult<bool, A::Error> {
        let res = match *self {
            MaybeDone::NotYet(ref mut a, ref a_tokens) => {
                if tokens.may_contain(a_tokens) {
                    a.poll(a_tokens)
                } else {
                    return Ok(false)
                }
            }
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

    fn collapse(&mut self) {
        match *self {
            MaybeDone::NotYet(ref mut a, _) => a.collapse(),
            _ => {}
        }
    }
}
