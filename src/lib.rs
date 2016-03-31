// #![feature(recover)]

extern crate crossbeam;

use std::any::Any;
use std::marker;
use std::mem;
use std::sync::Arc;

// use cell::AtomicCell;
// use slot::Slot;

// pub mod bufstream;
// mod buf;
// mod cell;
// pub mod channel;
// pub mod mio;
// pub mod promise;
// mod slot;
// pub mod stream;

pub use done::{done, Done};
pub use error::{PollError, PollResult, FutureError, FutureResult};
pub use failed::{failed, Failed};
pub use finished::{finished, Finished};
pub use lazy::{lazy, Lazy};
pub use map::Map;
pub use map_err::MapErr;

mod done;
mod error;
mod failed;
mod finished;
mod impls;
mod lazy;
mod map;
mod map_err;
mod util;

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
pub trait Future/*: Send + 'static*/ {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>>;

    fn cancel(&mut self);

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static,
              Self: Sized;

    fn schedule_boxed(&mut self, f: Box<Callback<Self::Item, Self::Error>>);

    fn await(&mut self) -> FutureResult<Self::Item, Self::Error>;

    fn boxed<'a>(self) -> Box<Future<Item=Self::Item, Error=Self::Error> + 'a>
        where Self: Sized + 'a
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

    // fn then<F, B>(self, f: F) -> Then<Self, B, F>
    //     where F: FnOnce(Result<Self::Item, Self::Error>) -> B + Send + 'static,
    //           B: IntoFuture,
    //           Self: Sized,
    // {
    //     Then {
    //         future: _Then::First(self, Some(f)),
    //     }
    // }
    //
    // // TODO: compare this to
    // //  ```
    // //  .then(|res| {
    // //      match res {
    // //          Ok(e) => Either::First(f(e).into_future()),
    // //          Err(e) => Either::Second(failed(e)),
    // //      }
    // //  })
    // // ```
    // fn and_then<F, B>(self, f: F) -> AndThen<Self, B, F>
    //     where F: FnOnce(Self::Item) -> B,
    //           B: IntoFuture<Error = Self::Error>,
    //           Self: Sized,
    // {
    //     AndThen {
    //         future: _AndThen::First(self, Some(f)),
    //     }
    // }
    //
    // // TODO: compare this to
    // //  ```
    // //  .then(|res| {
    // //      match res {
    // //          Ok(e) => Either::First(finished(e)),
    // //          Err(e) => Either::Second(f(e).into_future()),
    // //      }
    // //  })
    // // ```
    // fn or_else<F, B>(self, f: F) -> OrElse<Self, B, F>
    //     where F: FnOnce(Self::Error) -> B + Send + 'static,
    //           B: IntoFuture<Item = Self::Item>,
    //           Self: Sized,
    // {
    //     OrElse {
    //         future: _OrElse::First(self, Some(f)),
    //     }
    // }
    //
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
    //
    // // TODO: check this is the same as `and_then(|x| x)`
    // fn flatten(self) -> Flatten<Self>
    //     where Self::Item: IntoFuture,
    //           <<Self as Future>::Item as IntoFuture>::Error:
    //                 From<<Self as Future>::Error>,
    //           Self: Sized
    // {
    //     Flatten {
    //         state: _Flatten::First(self),
    //     }
    // }
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

pub struct Then<A, B, F> where B: IntoFuture {
    state: _Then<A, B::Future, F>,
}

enum _Then<A, B, F> {
    First(A, Option<F>),
    Second(B),
}

fn then<A, B, C, D, F>(a: PollResult<A, B>, f: F) -> PollResult<C, D>
    where F: FnOnce(Result<A, B>) -> C + Send + 'static,
          A: Send + 'static,
          B: Send + 'static,
{
    match a {
        Ok(e) => util::recover(|| f(Ok(e))),
        Err(PollError::Other(e)) => util::recover(|| f(Err(e))),
        Err(PollError::Panicked(e)) => Err(PollError::Panicked(e)),
        Err(PollError::Canceled) => Err(PollError::Canceled),
    }
}

impl<A, B, F> Future for Then<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: FnOnce(Result<A::Item, A::Error>) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self) -> Option<PollResult<B::Item, B::Error>> {
        let mut b = match self.state {
            _Then::First(ref mut a, ref mut f) => {
                let res = match a.poll() {
                    Some(res) => res,
                    None => return None,
                };
                let f = util::opt2poll(f.take());
                match f.and_then(|f| then(res, f)) {
                    Ok(b) => b.into_future(),
                    Err(e) => return Some(Err(e)),
                }
            }
            _Then::Second(ref mut b) => return b.poll(),
        };
        let ret = b.poll();
        self.state = _Then::Second(b);
        return ret
    }

    fn cancel(&mut self) {
        match self.state {
            _Then::First(ref mut a, _) => a.cancel(),
            _Then::Second(ref mut b) => b.cancel(),
        }
    }

    fn await(&mut self) -> FutureResult<B::Item, B::Error> {
        match self.state {
            _Then::First(ref mut a, ref mut f) => {
                let f = try!(util::opt2poll(f.take()));
                match a.await() {
                    Ok(e) => f(Ok(e)).into_future().await(),
                    Err(FutureError::Other(e)) => f(Err(e)).into_future().await(),
                    Err(FutureError::Canceled) => Err(FutureError::Canceled)
                }
            }
            _Then::Second(ref mut b) => b.await(),
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        // match self.future {
        //     _Then::First(ref mut a, ref mut f) => {
        //         let f = f.take();
        //         a.schedule(|r| {
        //             match then(r, f) {
        //                 Ok(b) => b.into_future().schedule(g),
        //                 Err(e) => g(Err(e)),
        //             }
        //         })
        //     }
        //     _Then::Second(ref mut b) => b.schedule(g)
        // }
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

// pub struct AndThen<A, B, F> where B: IntoFuture {
//     future: _AndThen<A, B::Future, F>,
// }
//
// enum _AndThen<A, B, F> {
//     First(A, Option<F>),
//     Second(B),
// }
//
// fn and_then<A, B, C, F>(a: FutureResult<A, B>, f: Option<F>) -> FutureResult<C, B>
//     where F: FnOnce(A) -> C + Send + 'static,
//           A: Send + 'static,
// {
//     match (f, a) {
//         (Some(f), Ok(e)) => recover(|| f(e)),
//         (Some(_), Err(e)) => Err(e),
//         (None, _) => Err(FutureError::Panicked)
//     }
// }
//
// impl<A, B, F> Future for AndThen<A, B, F>
//     where A: Future,
//           B: IntoFuture<Error = A::Error>,
//           F: FnOnce(A::Item) -> B + Send + 'static,
// {
//     type Item = B::Item;
//     type Error = B::Error;
//
//     fn poll(&mut self) -> Option<PollResult<B::Item, B::Error>> {
//         let mut b = match self.future {
//             _AndThen::First(ref mut a, ref mut f) => {
//                 let res = match a.poll() {
//                     Some(res) => res,
//                     None => return None,
//                 };
//                 match and_then(res, f.take()) {
//                     Ok(b) => b.into_future(),
//                     Err(e) => return Some(Err(e)),
//                 }
//             }
//             _AndThen::Second(ref mut b) => return b.poll(),
//         };
//         let ret = b.poll();
//         self.future = _AndThen::Second(b);
//         return ret
//     }
//
//     fn await(self) -> FutureResult<B::Item, B::Error> {
//         match self.future {
//             _AndThen::First(a, f) => {
//                 and_then(a.await(), f).and_then(|b| {
//                     b.into_future().await()
//                 })
//             }
//             _AndThen::Second(b) => b.await(),
//         }
//     }
//
//     fn schedule<G>(&mut self, g: G)
//         where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
//     {
//         match self.future {
//             _AndThen::First(ref mut a, ref mut f) => {
//                 let f = f.take();
//                 a.schedule(|r| {
//                     match and_then(r, f) {
//                         Ok(b) => b.into_future().schedule(g),
//                         Err(e) => g(Err(e)),
//                     }
//                 })
//             }
//             _AndThen::Second(ref mut b) => b.schedule(g)
//         }
//     }
//
//     fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
//         self.schedule(|r| cb.call(r))
//     }
// }
//
// pub struct OrElse<A, B, F> where B: IntoFuture {
//     future: _OrElse<A, B::Future, F>,
// }
//
// enum _OrElse<A, B, F> {
//     First(A, Option<F>),
//     Second(B),
// }
//
// fn or_else<A, B, C, F>(a: FutureResult<A, B>, f: Option<F>) -> FutureResult<A, C>
//     where F: FnOnce(B) -> C + Send + 'static,
//           B: Send + 'static,
// {
//     match (f, a) {
//         (Some(_), Ok(e)) => Ok(e),
//         (Some(f), Err(FutureError::Other(e))) => {
//             match recover::<_, _, i32>(|| f(e)) {
//                 Ok(e) => Err(FutureError::Other(e)),
//                 Err(FutureError::Panicked) => Err(FutureError::Panicked),
//                 Err(FutureError::Other(..)) => panic!(),
//             }
//         }
//         (Some(_), Err(FutureError::Panicked)) |
//         (None, _) => Err(FutureError::Panicked)
//     }
// }
//
// impl<A, B, F> Future for OrElse<A, B, F>
//     where A: Future,
//           B: IntoFuture<Item = A::Item>,
//           F: FnOnce(A::Error) -> B + Send + 'static,
// {
//     type Item = B::Item;
//     type Error = B::Error;
//
//     fn poll(&mut self) -> Option<PollResult<B::Item, B::Error>> {
//         let mut b = match self.future {
//             _OrElse::First(ref mut a, ref mut f) => {
//                 let res = match a.poll() {
//                     Some(res) => res,
//                     None => return None,
//                 };
//                 match or_else(res, f.take()) {
//                     Err(FutureError::Other(e)) => e.into_future(),
//                     Err(FutureError::Panicked) => {
//                         return Some(Err(FutureError::Panicked))
//                     }
//                     Ok(e) => return Some(Ok(e)),
//                 }
//             }
//             _OrElse::Second(ref mut b) => return b.poll(),
//         };
//         let ret = b.poll();
//         self.future = _OrElse::Second(b);
//         return ret
//     }
//
//     fn await(self) -> FutureResult<B::Item, B::Error> {
//         match self.future {
//             _OrElse::First(a, f) => {
//                 match or_else(a.await(), f) {
//                     Err(FutureError::Other(e)) => e.into_future().await(),
//                     Err(FutureError::Panicked) => Err(FutureError::Panicked),
//                     Ok(e) => Ok(e),
//                 }
//             }
//             _OrElse::Second(b) => b.await(),
//         }
//     }
//
//     fn schedule<G>(&mut self, g: G)
//         where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
//     {
//         match self.future {
//             _OrElse::First(ref mut a, ref mut f) => {
//                 let f = f.take();
//                 a.schedule(|r| {
//                     match or_else(r, f) {
//                         Err(FutureError::Other(e)) => e.into_future().schedule(g),
//                         Err(FutureError::Panicked) => g(Err(FutureError::Panicked)),
//                         Ok(e) => g(Ok(e)),
//                     }
//                 })
//             }
//             _OrElse::Second(ref mut b) => b.schedule(g)
//         }
//     }
//
//     fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
//         self.schedule(|r| cb.call(r))
//     }
// }
//
// pub struct Empty<T, E> {
//     _marker: marker::PhantomData<(T, E)>,
// }
//
// pub fn empty<T: Send + 'static, E: Send + 'static>() -> Empty<T, E> {
//     Empty { _marker: marker::PhantomData }
// }
//
// impl<T, E> Future for Empty<T, E>
//     where T: Send + 'static,
//           E: Send + 'static,
// {
//     type Item = T;
//     type Error = E;
//
//     fn poll(&mut self) -> Option<PollResult<T, E>> {
//         None
//     }
//
//     fn await(self) -> FutureResult<T, E> {
//         panic!("cannot ever successfully await() on Empty")
//     }
//
//     fn schedule<G>(&mut self, g: G)
//         where G: FnOnce(PollResult<T, E>) + Send + 'static
//     {
//         drop(g);
//     }
//
//     fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
//         drop(cb);
//     }
// }
//
// impl<T, E> Clone for Empty<T, E> {
//     fn clone(&self) -> Empty<T, E> {
//         Empty { _marker: marker::PhantomData }
//     }
// }
//
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
//
// pub struct Flatten<A> where A: Future, A::Item: IntoFuture {
//     state: _Flatten<A, <A::Item as IntoFuture>::Future>,
// }
//
// enum _Flatten<A, B> {
//     First(A),
//     Second(B),
// }
//
// impl<A> Future for Flatten<A>
//     where A: Future,
//           A::Item: IntoFuture,
//           <<A as Future>::Item as IntoFuture>::Error: From<<A as Future>::Error>
// {
//     type Item = <A::Item as IntoFuture>::Item;
//     type Error = <A::Item as IntoFuture>::Error;
//
//     fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
//         let mut b = match self.state {
//             _Flatten::First(ref mut a) => {
//                 match a.poll() {
//                     Some(Ok(a)) => a.into_future(),
//                     Some(Err(e)) => return Some(Err(e.map(From::from))),
//                     None => return None,
//                 }
//             }
//             _Flatten::Second(ref mut b) => return b.poll(),
//         };
//         let ret = b.poll();
//         self.state = _Flatten::Second(b);
//         return ret
//     }
//
//     fn await(self) -> FutureResult<Self::Item, Self::Error> {
//         match self.state {
//             _Flatten::First(a) => {
//                 match a.await() {
//                     Ok(b) => b.into_future().await(),
//                     Err(e) => Err(e.map(From::from)),
//                 }
//             }
//             _Flatten::Second(b) => b.await(),
//         }
//     }
//
//     fn schedule<G>(&mut self, g: G)
//         where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
//     {
//         match self.state {
//             _Flatten::First(ref mut a) => {
//                 a.schedule(|result| {
//                     match result {
//                         Ok(item) => item.into_future().schedule(g),
//                         Err(e) => g(Err(e.map(From::from))),
//                     }
//                 })
//             }
//             _Flatten::Second(ref mut b) => b.schedule(g),
//         }
//     }
//
//     fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
//         self.schedule(|r| cb.call(r))
//     }
// }
