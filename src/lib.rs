// pub mod bufstream;
// mod buf;
// pub mod channel;
// pub mod mio;
// pub mod stream;
mod cell;
mod slot;
mod util;

mod error;
pub use error::{PollError, PollResult, FutureError, FutureResult};

// Primitive futures
mod collect;
mod done;
mod empty;
mod failed;
mod finished;
mod lazy;
mod promise;
pub use collect::{collect, Collect};
pub use done::{done, Done};
pub use empty::{empty, Empty};
pub use failed::{failed, Failed};
pub use finished::{finished, Finished};
pub use lazy::{lazy, Lazy};
pub use promise::{promise, Promise, Complete};

// combinators
mod and_then;
mod chain;
mod flatten;
mod impls;
mod join;
mod map;
mod map_err;
mod or_else;
mod select;
mod then;
pub use and_then::AndThen;
pub use flatten::Flatten;
pub use join::Join;
pub use map::Map;
pub use map_err::MapErr;
pub use or_else::OrElse;
pub use select::Select;
pub use then::Then;

// streams
// pub mod stream;

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
    //       well what if you schedule() then drop, HUH?!
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

    fn map<F, U>(self, f: F) -> Map<Self, F>
        where F: FnOnce(Self::Item) -> U + Send + 'static,
              U: Send + 'static,
              Self: Sized,
    {
        assert_future::<U, Self::Error, _>(map::new(self, f))
    }

    fn map2<F, U>(self, f: F) -> Box<Future<Item=U, Error=Self::Error>>
        where F: FnOnce(Self::Item) -> U + Send + 'static,
              U: Send + 'static,
              Self: Sized,
    {
        self.then(|r| r.map(f)).boxed()
    }

    fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
        where F: FnOnce(Self::Error) -> E + Send + 'static,
              E: Send + 'static,
              Self: Sized,
    {
        assert_future::<Self::Item, E, _>(map_err::new(self, f))
    }

    fn map_err2<F, E>(self, f: F) -> Box<Future<Item=Self::Item, Error=E>>
        where F: FnOnce(Self::Error) -> E + Send + 'static,
              E: Send + 'static,
              Self: Sized,
    {
        self.then(|res| res.map_err(f)).boxed()
    }

    fn then<F, B>(self, f: F) -> Then<Self, B, F>
        where F: FnOnce(Result<Self::Item, Self::Error>) -> B + Send + 'static,
              B: IntoFuture,
              Self: Sized,
    {
        assert_future::<B::Item, B::Error, _>(then::new(self, f))
    }

    fn and_then<F, B>(self, f: F) -> AndThen<Self, B, F>
        where F: FnOnce(Self::Item) -> B + Send + 'static,
              B: IntoFuture<Error = Self::Error>,
              Self: Sized,
    {
        assert_future::<B::Item, Self::Error, _>(and_then::new(self, f))
    }

    fn and_then2<F, B>(self, f: F) -> Box<Future<Item=B::Item, Error=Self::Error>>
        where F: FnOnce(Self::Item) -> B + Send + 'static,
              B: IntoFuture<Error = Self::Error>,
              Self: Sized,
    {
        self.then(|res| {
            match res {
                Ok(e) => f(e).into_future().boxed(),
                Err(e) => failed(e).boxed(),
            }
        }).boxed()
    }

    fn or_else<F, B>(self, f: F) -> OrElse<Self, B, F>
        where F: FnOnce(Self::Error) -> B + Send + 'static,
              B: IntoFuture<Item = Self::Item>,
              Self: Sized,
    {
        assert_future::<Self::Item, B::Error, _>(or_else::new(self, f))
    }

    fn or_else2<F, B>(self, f: F) -> Box<Future<Item=B::Item, Error=B::Error>>
        where F: FnOnce(Self::Error) -> B + Send + 'static,
              B: IntoFuture<Item = Self::Item>,
              Self: Sized,
    {
        self.then(|res| {
            match res {
                Ok(e) => finished(e).boxed(),
                Err(e) => f(e).into_future().boxed(),
            }
        }).boxed()
    }

    fn select<B>(self, other: B) -> Select<Self, B::Future>
        where B: IntoFuture<Item=Self::Item, Error=Self::Error>,
              Self: Sized,
    {
        let f = select::new(self, other.into_future());
        assert_future::<Self::Item, Self::Error, _>(f)
    }

    fn join<B>(self, other: B) -> Join<Self, B::Future>
        where B: IntoFuture<Error=Self::Error>,
              Self: Sized,
    {
        let f = join::new(self, other.into_future());
        assert_future::<(Self::Item, B::Item), Self::Error, _>(f)
    }

    fn flatten(self) -> Flatten<Self>
        where Self::Item: IntoFuture,
              <<Self as Future>::Item as IntoFuture>::Error:
                    From<<Self as Future>::Error>,
              Self: Sized
    {
        let f = flatten::new(self);
        assert_future::<<<Self as Future>::Item as IntoFuture>::Item,
                        <<Self as Future>::Item as IntoFuture>::Error,
                        _>(f)
    }

    fn flatten2(self) -> Box<Future<Item=<<Self as Future>::Item as IntoFuture>::Item,
                                    Error=<<Self as Future>::Item as IntoFuture>::Error>>
        where Self::Item: IntoFuture,
              <<Self as Future>::Item as IntoFuture>::Error:
                    From<<Self as Future>::Error>,
              Self: Sized
    {
        self.then(|res| {
            match res {
                Ok(e) => e.into_future().boxed(),
                Err(e) => failed(From::from(e)).boxed(),
            }
        })
    }
}

fn assert_future<A, B, F>(t: F) -> F
    where F: Future<Item=A, Error=B>,
          A: Send + 'static,
          B: Send + 'static,
{
    t
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
