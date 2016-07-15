use std::sync::Arc;

use {Future, IntoFuture, Wake, Tokens};
use chain::Chain;

/// Future for the `or_else` combinator, chaining a computation onto the end of
/// a future which fails with an error.
///
/// This is created by this `Future::or_else` method.
pub struct OrElse<A, B, F> where A: Future, B: IntoFuture {
    state: Chain<A, B::Future, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> OrElse<A, B, F>
    where A: Future,
          B: IntoFuture<Item=A::Item>,
          F: Send + 'static,
{
    OrElse {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Future for OrElse<A, B, F>
    where A: Future,
          B: IntoFuture<Item=A::Item>,
          F: FnOnce(A::Error) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<Result<B::Item, B::Error>> {
        self.state.poll(tokens, |a, f| {
            match a {
                Ok(item) => Ok(Ok(item)),
                Err(e) => Ok(Err(f(e).into_future()))
            }
        })
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.state.schedule(wake)
    }

    fn tailcall(&mut self)
                -> Option<Box<Future<Item=Self::Item, Error=Self::Error>>> {
        self.state.tailcall()
    }
}
