//! Definition of the `TailFn` combinator, implementing tail-recursive loops.

use {Async, Future, IntoFuture, Poll};

/// The status of a `tail_fn` loop.
pub enum Tail<T, S> {
    /// Indicates that the loop has completed with output `T`.
    Done(T),

    /// Indicates that the loop function should be called again with input
    /// state `S`.
    Loop(S),
}

/// A future implementing a tail-recursive loop.
///
/// Created by the `tail_fn` function.
pub struct TailFn<A, F> {
    future: A,
    func: F,
}

/// Creates a new future implementing a tail-recursive loop.
///
/// The loop function is immediately called with `initial_state` and should
/// return a value that can be converted to a future. On successful completion,
/// this future should output a `Tail<T, S>` to indicate the status of the
/// loop.
///
/// `Tail::Done(T)` halts the loop and completes the future with output `T`.
///
/// `Tail::Loop(S)` reinvokes the loop function with state `S`. The returned
/// future will be subsequently polled for a new `Tail<T, S>` value.
///
/// # Examples
///
/// ```
/// use futures::future::{ok, tail_fn, Future, Ok, Tail};
/// use std::io::Error;
///
/// struct Client {
///     ping_count: u8,
/// }
///
/// impl Client {
///     fn new() -> Self {
///         Client { ping_count: 0 }
///     }
///
///     fn send_ping(self) -> Ok<Self, Error> {
///         ok(Client { ping_count: self.ping_count + 1 })
///     }
///
///     fn receive_pong(self) -> Ok<(Self, bool), Error> {
///         let done = self.ping_count >= 5;
///         ok((self, done))
///     }
/// }
///
/// let ping_til_done = tail_fn(Client::new(), |client| {
///     client.send_ping()
///         .and_then(|client| client.receive_pong())
///         .and_then(|(client, done)| {
///             if done {
///                 Ok(Tail::Done(client))
///             } else {
///                 Ok(Tail::Loop(client))
///             }
///         })
/// });
/// ```
pub fn tail_fn<S, T, I, A, F>(initial_state: S, mut func: F) -> TailFn<A, F>
    where F: FnMut(S) -> I,
          A: Future<Item = Tail<T, S>>,
          I: IntoFuture<Future = A, Item = A::Item, Error = A::Error>
{
    TailFn {
        future: func(initial_state).into_future(),
        func: func,
    }
}

impl<S, T, I, A, F> Future for TailFn<A, F>
    where F: FnMut(S) -> I,
          A: Future<Item = Tail<T, S>>,
          I: IntoFuture<Future = A, Item = A::Item, Error = A::Error>
{
    type Item = T;
    type Error = A::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let status = try_ready!(self.future.poll());
        match status {
            Tail::Done(x) => Ok(Async::Ready(x)),
            Tail::Loop(s) => {
                self.future = (self.func)(s).into_future();
                Ok(Async::NotReady)
            }
        }
    }
}
