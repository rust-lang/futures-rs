// #![feature(recover)]

extern crate crossbeam;

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
mod flatten;
mod impls;
mod map;
mod map_err;
mod or_else;
mod then;
pub use and_then::AndThen;
pub use flatten::Flatten;
pub use map::Map;
pub use map_err::MapErr;
pub use or_else::OrElse;
pub use then::Then;

pub trait IntoFuture/*: Send + 'static*/ {
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

// #[derive(PartialEq, Copy, Clone, Debug)]
// pub enum PollError<E> {
//     Panicked,
//     Canceled,
//     NotReady,
//     AlreadyDone,
//     Other(E),
// }
//
// impl<E> From<E> for PollError<E> {
//     fn from(e: E) -> PollError<E> {
//         PollError::Other(e)
//     }
// }
//
// impl<E> From<FutureError<E>> for PollError<E> {
//     fn from(e: FutureError<E>) -> PollError<E> {
//         match e {
//             FutureError::Panicked => PollError::Panicked,
//             FutureError::Canceled => PollError::Canceled,
//             FutureError::AlreadyDone => PollError::AlreadyDone,
//             FutureError::Other(e) => PollError::Other(e),
//         }
//     }
// }

// TODO: Send + 'static is annoying
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

    fn cancel(&mut self);

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static,
              Self: Sized;

    fn schedule_boxed(&mut self, f: Box<Callback<Self::Item, Self::Error>>);

    fn await(&mut self) -> FutureResult<Self::Item, Self::Error>;

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
    fn or_else<F, B>(self, f: F) -> OrElse<Self, B::Future, F>
        where F: FnOnce(Self::Error) -> B + Send + 'static,
              B: IntoFuture<Item = Self::Item>,
              Self: Sized,
    {
        or_else::new(self, f)
    }

    // fn select<B>(self, other: B) -> Select<Self, B::Future>
    //     where B: IntoFuture<Item=Self::Item, Error=Self::Error>,
    //           Self: Sized,
    // {
    //     Select {
    //         a: self,
    //         b: other.into_future(),
    //     }
    // }
    //
    // fn join<B>(self, other: B) -> Join<Self, B::Future>
    //     where B: IntoFuture<Error=Self::Error>,
    //           Self: Sized,
    // {
    //     Join {
    //         a: self,
    //         b: other.into_future(),
    //         a_res: None,
    //         b_res: None,
    //     }
    // }

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
// pub struct Select<A, B> {
//     a: A,
//     b: B,
// }
//
// impl<A, B> Future for Select<A, B>
//     where A: Future,
//           B: Future<Item=A::Item, Error=A::Error>,
// {
//     type Item = A::Item;
//     type Error = A::Error;
//
//     fn poll(&mut self) -> Option<PollResult<A::Item, A::Error>> {
//         self.a.poll().or_else(|| self.b.poll())
//     }
//
//     fn schedule<G>(&mut self, g: G)
//         where G: FnOnce(PollResult<A::Item, A::Error>) + Send + 'static
//     {
//         let cell1 = Arc::new(cell::AtomicCell::new(Some(g)));
//         let cell2 = cell1.clone();
//
//         self.a.schedule(move |result| {
//             let cell1 = cell1.borrow();
//             if let Some(g) = cell1.and_then(|mut b| b.take()) {
//                 g(result);
//             }
//         });
//         self.b.schedule(move |result| {
//             let cell2 = cell2.borrow();
//             if let Some(g) = cell2.and_then(|mut b| b.take()) {
//                 g(result);
//             }
//         });
//     }
//
//     fn schedule_boxed(&mut self, cb: Box<Callback<A::Item, A::Error>>) {
//         self.schedule(|r| cb.call(r))
//     }
// }
//
// pub struct Join<A, B> where A: Future, B: Future<Error=A::Error> {
//     a: A,
//     b: B,
//     a_res: Option<PollResult<A::Item, A::Error>>,
//     b_res: Option<PollResult<B::Item, A::Error>>,
// }
//
// impl<A, B> Future for Join<A, B>
//     where A: Future,
//           B: Future<Error=A::Error>,
// {
//     type Item = (A::Item, B::Item);
//     type Error = A::Error;
//
//     fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
//         let a = self.a_res.take().or_else(|| self.a.poll());
//         let b = self.b_res.take().or_else(|| self.b.poll());
//         match (a, b) {
//             // TODO: cancel the other?
//             (Some(Err(e)), _) |
//             (_, Some(Err(e))) => Some(Err(e)),
//
//             (None, _) |
//             (_, None) => None,
//             (Some(Ok(a)), Some(Ok(b))) => Some(Ok((a, b))),
//         }
//     }
//
//     fn schedule<G>(&mut self, g: G)
//         where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
//     {
//         match (self.a_res.take(), self.b_res.take()) {
//             (Some(a), Some(b)) => {
//                 return g(a.and_then(|a| b.map(|b| (a, b))))
//             }
//             (Some(Ok(a)), None) => {
//                 return self.b.schedule(|res| g(res.map(|b| (a, b))))
//             }
//             (None, Some(Ok(b))) => {
//                 return self.a.schedule(|res| g(res.map(|a| (a, b))))
//             }
//             // TODO: cancel other?
//             (Some(Err(e)), None) |
//             (None, Some(Err(e))) => return g(Err(e)),
//
//             (None, None) => {}
//         }
//
//         struct State<G, A, B, E> {
//             a: cell::AtomicCell<Option<PollResult<A, E>>>,
//             b: cell::AtomicCell<Option<PollResult<B, E>>>,
//             g: cell::AtomicCounter<G>,
//         }
//
//         let data1 = Arc::new(State::<G, A::Item, B::Item, A::Error> {
//             a: cell::AtomicCell::new(None),
//             b: cell::AtomicCell::new(None),
//             g: cell::AtomicCounter::new(g, 2),
//         });
//         let data2 = data1.clone();
//
//         impl<G, A, B, E> State<G, A, B, E>
//             where A: Send + 'static,
//                   B: Send + 'static,
//                   E: Send + 'static,
//                   G: FnOnce(PollResult<(A, B), E>),
//         {
//             fn finish(&self) {
//                 let g = match self.g.try_take() {
//                     Some(g) => g,
//                     None => return,
//                 };
//                 let a = self.a.borrow().unwrap().take().unwrap();
//                 let b = self.b.borrow().unwrap().take().unwrap();
//                 g(a.and_then(|a| b.map(|b| (a, b))))
//             }
//         }
//
//         self.a.schedule(move |result| {
//             // TODO: if we hit an error we should finish immediately
//             *data1.a.borrow().unwrap() = Some(result);
//             data1.finish();
//         });
//         self.b.schedule(move |result| {
//             *data2.b.borrow().unwrap() = Some(result);
//             data2.finish();
//         });
//     }
//
//     fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
//         self.schedule(|r| cb.call(r))
//     }
// }
//
// pub struct Collect<I> where I: Iterator, I::Item: Future {
//     remaining: Option<I>,
//     cur: Option<I::Item>,
//     result: Vec<<I::Item as Future>::Item>,
// }
//
// pub fn collect<I>(i: I) -> Collect<I::IntoIter>
//     where I: IntoIterator,
//           I::Item: Future,
//           I::IntoIter: Send + 'static,
// {
//     let mut i = i.into_iter();
//     Collect { cur: i.next(), remaining: Some(i), result: Vec::new() }
// }
//
// impl<I> Future for Collect<I>
//     where I: Iterator + Send + 'static,
//           I::Item: Future,
// {
//     type Item = Vec<<I::Item as Future>::Item>;
//     type Error = <I::Item as Future>::Error;
//
//     fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
//         let remaining = match self.remaining.as_mut() {
//             Some(i) => i,
//             None => return Some(Err(FutureError::Panicked)),
//         };
//         loop {
//             match self.cur.as_mut().map(Future::poll) {
//                 Some(Some(Ok(i))) => {
//                     self.result.push(i);
//                     self.cur = remaining.next();
//                 }
//                 Some(Some(Err(e))) => return Some(Err(e)),
//                 Some(None) => return None,
//                 None => return Some(Ok(mem::replace(&mut self.result, Vec::new()))),
//             }
//         }
//     }
//
//     fn await(self) -> FutureResult<Self::Item, Self::Error> {
//         let Collect { remaining, cur, mut result } = self;
//         let remaining = match remaining {
//             Some(i) => i,
//             None => return Err(FutureError::Panicked),
//         };
//         for item in cur.into_iter().chain(remaining) {
//             result.push(try!(item.await()));
//         }
//         Ok(result)
//     }
//
//     fn schedule<G>(&mut self, g: G)
//         where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
//     {
//         let mut result = mem::replace(&mut self.result, Vec::new());
//         let mut next = match self.cur.take() {
//             Some(next) => next,
//             None => return g(Ok(result)),
//         };
//         let mut remaining = match self.remaining.take() {
//             Some(i) => i,
//             None => return g(Err(FutureError::Panicked)),
//         };
//         next.schedule(move |res| {
//             let item = match res {
//                 Ok(i) => i,
//                 Err(e) => return g(Err(e)),
//             };
//             result.push(item);
//             Collect {
//                 cur: remaining.next(),
//                 remaining: Some(remaining),
//                 result: result,
//             }.schedule(g);
//         })
//     }
//
//     fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
//         self.schedule(|r| cb.call(r))
//     }
// }
