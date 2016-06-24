use std::sync::Arc;
use std::mem;

use {PollResult, Wake, Future, Tokens, empty};
use util::{self, Collapsed};

/// Future for the `select` combinator, waiting for one of two futures to
/// complete.
///
/// This is created by this `Future::select` method.
pub struct Select<A, B> where A: Future, B: Future<Item=A::Item, Error=A::Error> {
    inner: Option<(Collapsed<A>, Collapsed<B>)>,
    a_tokens: Tokens,
    b_tokens: Tokens,
}

/// Future yielded as the second result in a `Select` future.
///
/// This sentinel future represents the completion of the second future to a
/// `select` which finished second.
pub struct SelectNext<A, B> where A: Future, B: Future<Item=A::Item, Error=A::Error> {
    inner: OneOf<A, B>,
}

enum OneOf<A, B> where A: Future, B: Future {
    A(Collapsed<A>),
    B(Collapsed<B>),
}

pub fn new<A, B>(a: A, b: B) -> Select<A, B>
    where A: Future,
          B: Future<Item=A::Item, Error=A::Error>
{
    let a = Collapsed::Start(a);
    let b = Collapsed::Start(b);
    Select {
        inner: Some((a, b)),
        a_tokens: Tokens::all(),
        b_tokens: Tokens::all(),
    }
}

impl<A, B> Future for Select<A, B>
    where A: Future,
          B: Future<Item=A::Item, Error=A::Error>,
{
    type Item = (A::Item, SelectNext<A, B>);
    type Error = (A::Error, SelectNext<A, B>);

    fn poll(&mut self, tokens: &Tokens)
            -> Option<PollResult<Self::Item, Self::Error>> {
        let (ret, is_a) = match self.inner {
            Some((_, ref mut b)) if !self.a_tokens.may_contain(tokens) => {
                match b.poll(&(tokens & &self.b_tokens)) {
                    Some(b) => (b, false),
                    None => return None,
                }
            }
            Some((ref mut a, ref mut b)) => {
                match a.poll(&(tokens & &self.a_tokens)) {
                    Some(a) => (a, true),
                    None if !self.b_tokens.may_contain(tokens) => return None,
                    None => {
                        match b.poll(&(tokens & &self.b_tokens)) {
                            Some(b) => (b, false),
                            None => return None,
                        }
                    }
                }
            }
            None => return Some(Err(util::reused())),
        };

        let (a, b) = self.inner.take().unwrap();
        let next = if is_a {OneOf::B(b)} else {OneOf::A(a)};
        let next = SelectNext { inner: next };
        Some(match ret {
            Ok(a) => Ok((a, next)),
            Err(e) => Err(e.map(move |e| (e, next))),
        })
    }

    fn schedule(&mut self, wake: Arc<Wake>) -> Tokens {
        match self.inner {
            Some((ref mut a, ref mut b)) => {
                self.a_tokens = a.schedule(wake.clone());
                self.b_tokens = b.schedule(wake.clone());
                &self.a_tokens | &self.b_tokens
            }
            None => util::done(wake),
        }
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        if let Some((ref mut a, ref mut b)) = self.inner {
            a.collapse();
            b.collapse();
        }
        None
    }
}

impl<A, B> Future for SelectNext<A, B>
    where A: Future,
          B: Future<Item=A::Item, Error=A::Error>,
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<PollResult<Self::Item, Self::Error>> {
        match self.inner {
            OneOf::A(ref mut a) => a.poll(tokens),
            OneOf::B(ref mut b) => b.poll(tokens),
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) -> Tokens {
        match self.inner {
            OneOf::A(ref mut a) => a.schedule(wake),
            OneOf::B(ref mut b) => b.schedule(wake),
        }
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        match self.inner {
            OneOf::A(ref mut a) => a.collapse(),
            OneOf::B(ref mut b) => b.collapse(),
        }
        match self.inner {
            OneOf::A(Collapsed::Tail(ref mut a)) |
            OneOf::B(Collapsed::Tail(ref mut a)) => {
                Some(mem::replace(a, Box::new(empty())))
            }
            _ => None,
        }
    }
}
