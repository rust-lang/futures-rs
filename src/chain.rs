use std::sync::Arc;
use std::mem;

use util::{self, Collapsed};
use {Future, Wake, Tokens, TOKENS_ALL};

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

    pub fn poll<F>(&mut self, tokens: &Tokens, f: F)
                  -> Option<Result<B::Item, B::Error>>
        where F: FnOnce(Result<A::Item, A::Error>, C)
                        -> Result<Result<B::Item, B>, B::Error> + Send + 'static,
    {
        let a_result = match *self {
            Chain::First(ref mut a, _) => {
                match a.poll(tokens) {
                    Some(a) => a,
                    None => return None,
                }
            }
            Chain::Second(ref mut b) => return b.poll(tokens),
            Chain::Done => panic!("cannot poll a chained future twice"),
        };
        let data = match mem::replace(self, Chain::Done) {
            Chain::First(_, c) => c,
            _ => panic!(),
        };
        match f(a_result, data) {
            Ok(Ok(e)) => Some(Ok(e)),
            Ok(Err(mut b)) => {
                let ret = b.poll(&TOKENS_ALL);
                *self = Chain::Second(b);
                ret
            }
            Err(e) => Some(Err(e)),
        }
    }

    pub fn schedule(&mut self, wake: &Arc<Wake>) {
        match *self {
            Chain::First(ref mut a, _) => a.schedule(wake),
            Chain::Second(ref mut b) => b.schedule(wake),
            Chain::Done => util::done(wake),
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
