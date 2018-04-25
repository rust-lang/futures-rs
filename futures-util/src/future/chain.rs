use core::mem;

use futures_core::{Future, PollResult, Poll};
use futures_core::task;

#[must_use = "futures do nothing unless polled"]
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

    pub fn poll<F>(&mut self, cx: &mut task::Context, f: F) -> PollResult<B::Item, B::Error>
        where F: FnOnce(Result<A::Item, A::Error>, C)
                        -> Result<Result<B::Item, B>, B::Error>,
    {
        let a_result = match *self {
            Chain::First(ref mut a, _) => {
                match a.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(r) => r,
                }
            }
            Chain::Second(ref mut b) => return b.poll(cx),
            Chain::Done => panic!("cannot poll a chained future twice"),
        };
        let data = match mem::replace(self, Chain::Done) {
            Chain::First(_, c) => c,
            _ => panic!(),
        };
        match f(a_result, data) {
            Err(e) => Poll::Ready(Err(e)),
            Ok(Ok(e)) => Poll::Ready(Ok(e)),
            Ok(Err(mut b)) => {
                let ret = b.poll(cx);
                *self = Chain::Second(b);
                ret
            }
        }
    }
}
