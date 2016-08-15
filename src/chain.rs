use std::mem;

use {Future, Poll};

pub enum Chain<A, B, C> where A: Future, B: 'static {
    First(A, C),
    Second(B),
    Done,
}

impl<A, B, C> Chain<A, B, C>
    where A: Future,
          B: Future,
          C: 'static,
{
    pub fn new(a: A, c: C) -> Chain<A, B, C> {
        Chain::First(a, c)
    }

    pub fn poll<F>(&mut self, f: F) -> Poll<B::Item, B::Error>
        where F: FnOnce(Result<A::Item, A::Error>, C)
                        -> Result<Result<B::Item, B>, B::Error> + 'static,
    {
        let a_result = match *self {
            Chain::First(ref mut a, _) => try_poll!(a.poll()),
            Chain::Second(ref mut b) => return b.poll(),
            Chain::Done => panic!("cannot poll a chained future twice"),
        };
        let data = match mem::replace(self, Chain::Done) {
            Chain::First(_, c) => c,
            _ => panic!(),
        };
        match f(a_result, data) {
            Ok(Ok(e)) => Poll::Ok(e),
            Ok(Err(mut b)) => {
                let ret = b.poll();
                *self = Chain::Second(b);
                ret
            }
            Err(e) => Poll::Err(e),
        }
    }
}
