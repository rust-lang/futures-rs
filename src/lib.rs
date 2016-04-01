// #![feature(recover)]

// use cell::AtomicCell;
// use slot::Slot;

// pub mod bufstream;
// mod buf;
mod cell;
// pub mod channel;
// pub mod mio;
// pub mod promise;
mod slot;
// pub mod stream;
mod util;

mod error;
pub use error::{PollError, PollResult, FutureError, FutureResult};

// Primitive futures
mod done;
mod empty;
mod failed;
mod finished;
mod lazy;
pub use done::{done, Done};
pub use empty::{empty, Empty};
pub use failed::{failed, Failed};
pub use finished::{finished, Finished};
pub use lazy::{lazy, Lazy};

// combinators
mod and_then;
mod chain;
// mod collect;
mod flatten;
mod impls;
mod map;
mod map_err;
mod or_else;
mod then;
mod join;
mod select;
pub use select::Select;
pub use and_then::AndThen;
// pub use collect::{collect, Collect};
pub use flatten::Flatten;
pub use map::Map;
pub use map_err::MapErr;
pub use or_else::OrElse;
pub use then::Then;
pub use join::Join;

// TODO: Send + 'static is annoying, but required by cancel and_then, document
// TODO: not object safe
//
// FINISH CONDITIONS
//      - poll() return Some
//      - await() is called
//      - schedule() is called
//      - schedule_boxed() is called
//
// BAD:
//      - doing any finish condition after an already called finish condition
//
// WHAT HAPPENS
//      - panic?
pub trait Future: Send + 'static {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>>;

    // TODO: why is this not drop()
    fn cancel(&mut self);

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static,
              Self: Sized;

    fn schedule_boxed(&mut self, f: Box<Callback<Self::Item, Self::Error>>);

    // TODO: why can't this be in this lib?
    // fn await(&mut self) -> FutureResult<Self::Item, Self::Error>;

    fn boxed(self) -> Box<Future<Item=Self::Item, Error=Self::Error>>
        where Self: Sized
    {
        Box::new(self)
    }

    // TODO: compare this to `.then(|x| x.map(f))`
    fn map<F, U>(self, f: F) -> Map<Self, F>
        where F: FnOnce(Self::Item) -> U + Send + 'static,
              Self: Sized,
    {
        map::new(self, f)
    }

    // TODO: compare this to `.then(|x| x.map_err(f))`
    fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
        where F: FnOnce(Self::Error) -> E + Send + 'static,
              E: Send + 'static,
              Self: Sized,
    {
        map_err::new(self, f)
    }

    fn then<F, B>(self, f: F) -> Then<Self, B::Future, F>
        where F: FnOnce(Result<Self::Item, Self::Error>) -> B + Send + 'static,
              B: IntoFuture,
              Self: Sized,
    {
        then::new(self, f)
    }

    // TODO: compare this to
    //  ```
    //  .then(|res| {
    //      match res {
    //          Ok(e) => Either::First(f(e).into_future()),
    //          Err(e) => Either::Second(failed(e)),
    //      }
    //  })
    // ```
    fn and_then<F, B>(self, f: F) -> AndThen<Self, B::Future, F>
        where F: FnOnce(Self::Item) -> B + Send + 'static,
              B: IntoFuture<Error = Self::Error>,
              Self: Sized,
    {
        and_then::new(self, f)
    }

    // TODO: compare this to
    //  ```
    //  .then(|res| {
    //      match res {
    //          Ok(e) => Either::First(finished(e)),
    //          Err(e) => Either::Second(f(e).into_future()),
    //      }
    //  })
    // ```
    fn or_else<F, B>(self, f: F) -> OrElse<Self, B, F>
        where F: FnOnce(Self::Error) -> B + Send + 'static,
              B: IntoFuture<Item = Self::Item>,
              Self: Sized,
    {
        or_else::new(self, f)
    }

    fn select<B>(self, other: B) -> Select<Self, B::Future>
        where B: IntoFuture<Item=Self::Item, Error=Self::Error>,
              Self: Sized,
    {
        select::new(self, other.into_future())
    }

    fn join<B>(self, other: B) -> Join<Self, B::Future>
        where B: IntoFuture<Error=Self::Error>,
              Self: Sized,
    {
        join::new(self, other.into_future())
    }

    // TODO: check this is the same as `and_then(|x| x)`
    fn flatten(self) -> Flatten<Self>
        where Self::Item: IntoFuture,
              <<Self as Future>::Item as IntoFuture>::Error:
                    From<<Self as Future>::Error>,
              Self: Sized
    {
        flatten::new(self)
    }
}

pub trait Callback<T, E>: Send + 'static {
    fn call(self: Box<Self>, result: PollResult<T, E>);
}

impl<F, T, E> Callback<T, E> for F
    where F: FnOnce(PollResult<T, E>) + Send + 'static
{
    fn call(self: Box<F>, result: PollResult<T, E>) {
        (*self)(result)
    }
}

pub trait IntoFuture: Send + 'static {
    type Future: Future<Item=Self::Item, Error=Self::Error>;
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn into_future(self) -> Self::Future;
}

impl<F: Future> IntoFuture for F {
    type Future = F;
    type Item = F::Item;
    type Error = F::Error;

    fn into_future(self) -> F {
        self
    }
}
