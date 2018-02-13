use core::mem;

use futures_core::{Future, Poll, Async};

#[derive(Debug)]
pub enum Chain<A, B, C> where A: Future {
    First(A, C),
    Second(B),
    Done,
}

impl<A, B, C> Chain<A, B, C>
    where A: Future,
          B: Future,
{
    pub fn new(a: A, c: C) -> Chain<A, B, C> {
        Chain::First(a, c)
    }

    pub unsafe fn poll_unsafe<F>(&mut self, f: F) -> Poll<B::Item, B::Error>
        where F: FnOnce(Result<A::Item, A::Error>, C)
                        -> Result<Result<B::Item, B>, B::Error>,
    {
        let a_result = match *self {
            Chain::First(ref mut a, _) => {
                match a.poll() {
                    Ok(Async::Pending) => return Ok(Async::Pending),
                    Ok(Async::Ready(t)) => Ok(t),
                    Err(e) => Err(e),
                }
            }
            Chain::Second(ref mut b) => return b.poll(),
            Chain::Done => panic!("cannot poll a chained future twice"),
        };
        let data = match mem::replace(self, Chain::Done) {
            Chain::First(_, c) => c,
            _ => panic!(),
        };
        match f(a_result, data)? {
            Ok(e) => Ok(Async::Ready(e)),
            Err(mut b) => {
                let ret = b.poll();
                *self = Chain::Second(b);
                ret
            }
        }
    }
}
