use std::mem;

use util::Collapsed;
use {Future, Task, Poll};

pub enum Chain<A, B, C> where A: Future, B: Send + 'static {
    First(Collapsed<A>, C),
    Second(B),
    Done,
}

impl<A, B, C> Chain<A, B, C>
    where A: Future,
          B: Future,
          C: Send + 'static,
{
    pub fn new(a: A, c: C) -> Chain<A, B, C> {
        Chain::First(Collapsed::Start(a), c)
    }

    pub fn poll<F>(&mut self, task: &mut Task, f: F) -> Poll<B::Item, B::Error>
        where F: FnOnce(Result<A::Item, A::Error>, C)
                        -> Result<Result<B::Item, B>, B::Error> + Send + 'static,
    {
        let a_result = match *self {
            Chain::First(ref mut a, _) => try_poll!(a.poll(task)),
            Chain::Second(ref mut b) => return b.poll(task),
            Chain::Done => panic!("cannot poll a chained future twice"),
        };
        let data = match mem::replace(self, Chain::Done) {
            Chain::First(_, c) => c,
            _ => panic!(),
        };
        match f(a_result, data) {
            Ok(Ok(e)) => Poll::Ok(e),
            Ok(Err(mut b)) => {
                let ret = b.poll(task);
                *self = Chain::Second(b);
                ret
            }
            Err(e) => Poll::Err(e),
        }
    }

    pub fn schedule(&mut self, task: &mut Task) {
        match *self {
            Chain::First(ref mut a, _) => a.schedule(task),
            Chain::Second(ref mut b) => b.schedule(task),
            Chain::Done => task.notify(),
        }
    }

    pub fn tailcall(&mut self)
                    -> Option<Box<Future<Item=B::Item, Error=B::Error>>> {
        match *self {
            Chain::First(ref mut a, _) => {
                a.collapse();
                None
            }
            Chain::Second(ref mut b) => b.tailcall(),
            Chain::Done => None,
        }
    }
}
