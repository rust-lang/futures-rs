// #![feature(recover)]

extern crate crossbeam;

use std::error::Error;
use std::marker;
use std::mem;
use std::sync::Arc;

use cell::AtomicCell;
// use slot::Slot;

// pub mod bufstream;
pub mod buf;
pub mod cell;
// pub mod channel;
// pub mod mio;
pub mod promise;
pub mod slot;
// pub mod stream;

fn recover<F, R, E>(f: F) -> FutureResult<R, E>
    where F: FnOnce() -> R + Send + 'static
{
    // use std::panic::{recover, AssertRecoverSafe};
    //
    // recover(AssertRecoverSafe(f)).map_err(|_| FutureError::Panicked)
    Ok(f())
}

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

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FutureError<E> {
    Panicked, // TODO: Box<Any + Send> payload?
    Other(E),
}

impl<E> From<E> for FutureError<E> {
    fn from(e: E) -> FutureError<E> {
        FutureError::Other(e)
    }
}

impl<E> FutureError<E> {
    pub fn canceled() -> FutureError<E> where E: From<Cancel> {
        FutureError::Other(E::from(Cancel))
    }

    fn map<E2, F: FnOnce(E) -> E2>(self, f: F) -> FutureError<E2> {
        match self {
            FutureError::Panicked => FutureError::Panicked,
            FutureError::Other(e) => FutureError::Other(f(e)),
        }
    }
}

pub struct Cancel;

// pub type PollResult<T, E> = Result<T, PollError<E>>;
pub type FutureResult<T, E> = Result<T, FutureError<E>>;

// TODO: Send + 'static is annoying
// TODO: not object safe
pub trait Future/*: Send + 'static*/ {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn poll(&mut self) -> Option<FutureResult<Self::Item, Self::Error>>;

    // fn cancel(&mut self);

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(FutureResult<Self::Item, Self::Error>) + Send + 'static,
              Self: Sized;

    fn schedule_boxed(&mut self, f: Box<Callback<Self::Item, Self::Error>>);

    fn await(self) -> FutureResult<Self::Item, Self::Error>
        where Self: Sized
    {
        loop {}
        // let slot = Arc::new((Slot::new(None), AtomicCell::new(None)));
        // let slot2 = slot.clone();
        // self.schedule(move |result| {
        //     *slot2.1.borrow().unwrap() = Some(result);
        //     slot2.0.try_produce(()).ok().unwrap();
        // });
        // unit_await(&slot.0);
        // let mut slot = slot.1.borrow().unwrap();
        // return slot.take().unwrap();
        //
        // fn unit_await(p: &Slot<()>) {
        //     mio::INNER.with(|l| l.await(p))
        // }
    }

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
        Map {
            future: self,
            f: Some(f),
        }
    }

    // TODO: compare this to `.then(|x| x.map_err(f))`
    fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
        where F: FnOnce(Self::Error) -> E + Send + 'static,
              E: Send + 'static,
              Self: Sized,
    {
        MapErr {
            future: self,
            f: Some(f),
        }
    }

    fn then<F, B>(self, f: F) -> Then<Self, B, F>
        where F: FnOnce(Result<Self::Item, Self::Error>) -> B + Send + 'static,
              B: IntoFuture,
              Self: Sized,
    {
        Then {
            future: _Then::First(self, Some(f)),
        }
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
    fn and_then<F, B>(self, f: F) -> AndThen<Self, B, F>
        where F: FnOnce(Self::Item) -> B,
              B: IntoFuture<Error = Self::Error>,
              Self: Sized,
    {
        AndThen {
            future: _AndThen::First(self, Some(f)),
        }
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
        OrElse {
            future: _OrElse::First(self, Some(f)),
        }
    }

    fn select<B>(self, other: B) -> Select<Self, B::Future>
        where B: IntoFuture<Item=Self::Item, Error=Self::Error>,
              Self: Sized,
    {
        Select {
            a: self,
            b: other.into_future(),
        }
    }

    fn join<B>(self, other: B) -> Join<Self, B::Future>
        where B: IntoFuture<Error=Self::Error>,
              Self: Sized,
    {
        Join {
            a: self,
            b: other.into_future(),
            a_res: None,
            b_res: None,
        }
    }

    // TODO: check this is the same as `and_then(|x| x)`
    fn flatten(self) -> Flatten<Self>
        where Self::Item: IntoFuture,
              <<Self as Future>::Item as IntoFuture>::Error:
                    From<<Self as Future>::Error>,
              Self: Sized
    {
        Flatten {
            state: _Flatten::First(self),
        }
    }

    // // fn cancellable(self) -> Cancellable<Self> where Self: Sized {
    // //     Cancellable {
    // //         future: self,
    // //         canceled: false,
    // //     }
    // // }
}

impl<F: ?Sized + Future> Future for Box<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Option<FutureResult<Self::Item, Self::Error>> {
        (**self).poll()
    }

    // fn cancel(&mut self) {
    //     (**self).cancel()
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<Self::Item, Self::Error>) + Send + 'static,
    {
        (**self).schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self, f: Box<Callback<Self::Item, Self::Error>>) {
        (**self).schedule_boxed(f)
    }
}

impl<'a, F: ?Sized + Future> Future for &'a mut F {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Option<FutureResult<Self::Item, Self::Error>> {
        (**self).poll()
    }

    // fn cancel(&mut self) {
    //     (**self).cancel()
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<Self::Item, Self::Error>) + Send + 'static,
    {
        (**self).schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self, f: Box<Callback<Self::Item, Self::Error>>) {
        (**self).schedule_boxed(f)
    }
}

pub trait Callback<T, E>: Send + 'static {
    fn call(self: Box<Self>, result: FutureResult<T, E>);
}

impl<F, T, E> Callback<T, E> for F
    where F: FnOnce(FutureResult<T, E>) + Send + 'static
{
    fn call(self: Box<F>, result: FutureResult<T, E>) {
        (*self)(result)
    }
}

#[derive(Clone)]
pub struct FutureResult2<T, E> {
    inner: FutureResult<T, E>,
}

impl<T, E> FutureResult2<T, E> {
    fn take(&mut self) -> FutureResult<T, E> {
        mem::replace(&mut self.inner, Err(FutureError::Panicked))
    }
}

impl<T, E> IntoFuture for Result<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Future = FutureResult2<T, E>;
    type Item = T;
    type Error = E;

    fn into_future(self) -> FutureResult2<T, E> {
        let inner = self.map_err(FutureError::Other);
        FutureResult2 { inner: inner }
    }
}

impl<T, E> Future for FutureResult2<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<FutureResult<T, E>> {
        Some(self.take())
    }

    fn await(mut self) -> FutureResult<T, E> {
        self.take()
    }

    // fn cancel(&mut self) {}

    fn schedule<F>(&mut self, f: F)
        where F: FnOnce(FutureResult<Self::Item, Self::Error>) + Send + 'static
    {
        f(self.take())
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r));
    }
}

pub struct Map<A, F> {
    future: A,
    f: Option<F>,
}

fn map<T, U, E, F>(result: FutureResult<T, E>,
                   f: Option<F>)
                   -> FutureResult<U, E>
    where F: FnOnce(T) -> U + Send + 'static,
          T: Send + 'static,
{
    match (f, result) {
        (Some(f), Ok(e)) => recover(|| f(e)),
        (None, Ok(..)) => panic!("already done but got success"),
        (_, Err(e)) => Err(e),
    }
}

impl<U, A, F> Future for Map<A, F>
    where A: Future,
          F: FnOnce(A::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = U;
    type Error = A::Error;

    fn poll(&mut self) -> Option<FutureResult<U, A::Error>> {
        self.future.poll().map(|res| map(res, self.f.take()))
    }

    // fn cancel(&mut self) {
    //     self.future.cancel()
    // }

    fn await(self) -> FutureResult<U, A::Error> {
        map(self.future.await(), self.f)
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<U, A::Error>) + Send + 'static
    {
        let f = self.f.take();
        self.future.schedule(|result| g(map(result, f)))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r));
    }
}

pub struct MapErr<A, F> {
    future: A,
    f: Option<F>,
}

fn map_err<T, U, E, F>(result: FutureResult<T, E>,
                       f: Option<F>)
                       -> FutureResult<T, U>
    where F: FnOnce(E) -> U + Send + 'static,
          T: Send + 'static,
          E: Send + 'static,
{
    match (f, result) {
        (Some(f), Err(FutureError::Other(e))) => {
            recover(|| f(e)).and_then(|e| Err(FutureError::Other(e)))
        }
        (None, Err(FutureError::Other(_))) => panic!("real return but success"),
        (_, Err(FutureError::Panicked)) => Err(FutureError::Panicked),
        (_, Ok(e)) => Ok(e),
    }
}

impl<U, A, F> Future for MapErr<A, F>
    where A: Future,
          F: FnOnce(A::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = A::Item;
    type Error = U;

    fn poll(&mut self) -> Option<FutureResult<A::Item, U>> {
        self.future.poll().map(|res| map_err(res, self.f.take()))
    }

    // fn cancel(&mut self) {
    //     self.future.cancel()
    // }

    fn await(self) -> FutureResult<A::Item, U> {
        map_err(self.future.await(), self.f)
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<A::Item, U>) + Send + 'static
    {
        let f = self.f.take();
        self.future.schedule(|result| g(map_err(result, f)))
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r));
    }
}

pub struct Then<A, B, F> where B: IntoFuture {
    future: _Then<A, B::Future, F>,
}

enum _Then<A, B, F> {
    First(A, Option<F>),
    Second(B),
}

fn then<A, B, C, D, F>(a: FutureResult<A, B>, f: Option<F>) -> FutureResult<C, D>
    where F: FnOnce(Result<A, B>) -> C + Send + 'static,
          A: Send + 'static,
          B: Send + 'static,
{
    match (f, a) {
        (Some(f), Ok(e)) => recover(|| f(Ok(e))),
        (Some(f), Err(FutureError::Other(e))) => recover(|| f(Err(e))),

        (_, Err(FutureError::Panicked)) |
        (None, _) => Err(FutureError::Panicked)
    }
}

impl<A, B, F> Future for Then<A, B, F>
    where A: Future,
          B: IntoFuture,
          F: FnOnce(Result<A::Item, A::Error>) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    // TODO: is into_future() called outside the recover()
    fn poll(&mut self) -> Option<FutureResult<B::Item, B::Error>> {
        let mut b = match self.future {
            _Then::First(ref mut a, ref mut f) => {
                let res = match a.poll() {
                    Some(res) => res,
                    None => return None,
                };
                match then(res, f.take()) {
                    Ok(b) => b.into_future(),
                    Err(e) => return Some(Err(e)),
                }
            }
            _Then::Second(ref mut b) => return b.poll(),
        };
        let ret = b.poll();
        self.future = _Then::Second(b);
        return ret
    }

    // TODO: should the closure be able to recover this cancel?
    //       aka, if you cancel `a`, the closure will currently run and allow
    //       recovery to continue on to `b`
    // fn cancel(&mut self) {
    //     match self.future {
    //         _Then::First(ref mut a, _) => a.cancel(),
    //         _Then::Second(ref mut b) => b.cancel(),
    //     }
    //     self.canceled = true;
    // }

    fn await(self) -> FutureResult<B::Item, B::Error> {
        match self.future {
            _Then::First(a, f) => {
                then(a.await(), f).and_then(|b| {
                    b.into_future().await()
                })
            }
            _Then::Second(b) => b.await(),
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<B::Item, B::Error>) + Send + 'static
    {
        match self.future {
            _Then::First(ref mut a, ref mut f) => {
                let f = f.take();
                a.schedule(|r| {
                    match then(r, f) {
                        Ok(b) => b.into_future().schedule(g),
                        Err(e) => g(Err(e)),
                    }
                })
            }
            _Then::Second(ref mut b) => b.schedule(g)
        }
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

pub struct AndThen<A, B, F> where B: IntoFuture {
    future: _AndThen<A, B::Future, F>,
}

enum _AndThen<A, B, F> {
    First(A, Option<F>),
    Second(B),
}

fn and_then<A, B, C, F>(a: FutureResult<A, B>, f: Option<F>) -> FutureResult<C, B>
    where F: FnOnce(A) -> C + Send + 'static,
          A: Send + 'static,
{
    match (f, a) {
        (Some(f), Ok(e)) => recover(|| f(e)),
        (Some(_), Err(e)) => Err(e),
        (None, _) => Err(FutureError::Panicked)
    }
}

impl<A, B, F> Future for AndThen<A, B, F>
    where A: Future,
          B: IntoFuture<Error = A::Error>,
          F: FnOnce(A::Item) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self) -> Option<FutureResult<B::Item, B::Error>> {
        let mut b = match self.future {
            _AndThen::First(ref mut a, ref mut f) => {
                let res = match a.poll() {
                    Some(res) => res,
                    None => return None,
                };
                match and_then(res, f.take()) {
                    Ok(b) => b.into_future(),
                    Err(e) => return Some(Err(e)),
                }
            }
            _AndThen::Second(ref mut b) => return b.poll(),
        };
        let ret = b.poll();
        self.future = _AndThen::Second(b);
        return ret
    }

    fn await(self) -> FutureResult<B::Item, B::Error> {
        match self.future {
            _AndThen::First(a, f) => {
                and_then(a.await(), f).and_then(|b| {
                    b.into_future().await()
                })
            }
            _AndThen::Second(b) => b.await(),
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<B::Item, B::Error>) + Send + 'static
    {
        match self.future {
            _AndThen::First(ref mut a, ref mut f) => {
                let f = f.take();
                a.schedule(|r| {
                    match and_then(r, f) {
                        Ok(b) => b.into_future().schedule(g),
                        Err(e) => g(Err(e)),
                    }
                })
            }
            _AndThen::Second(ref mut b) => b.schedule(g)
        }
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

pub struct OrElse<A, B, F> where B: IntoFuture {
    future: _OrElse<A, B::Future, F>,
}

enum _OrElse<A, B, F> {
    First(A, Option<F>),
    Second(B),
}

fn or_else<A, B, C, F>(a: FutureResult<A, B>, f: Option<F>) -> FutureResult<A, C>
    where F: FnOnce(B) -> C + Send + 'static,
          B: Send + 'static,
{
    match (f, a) {
        (Some(_), Ok(e)) => Ok(e),
        (Some(f), Err(FutureError::Other(e))) => {
            match recover::<_, _, i32>(|| f(e)) {
                Ok(e) => Err(FutureError::Other(e)),
                Err(FutureError::Panicked) => Err(FutureError::Panicked),
                Err(FutureError::Other(..)) => panic!(),
            }
        }
        (Some(_), Err(FutureError::Panicked)) |
        (None, _) => Err(FutureError::Panicked)
    }
}

impl<A, B, F> Future for OrElse<A, B, F>
    where A: Future,
          B: IntoFuture<Item = A::Item>,
          F: FnOnce(A::Error) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self) -> Option<FutureResult<B::Item, B::Error>> {
        let mut b = match self.future {
            _OrElse::First(ref mut a, ref mut f) => {
                let res = match a.poll() {
                    Some(res) => res,
                    None => return None,
                };
                match or_else(res, f.take()) {
                    Err(FutureError::Other(e)) => e.into_future(),
                    Err(FutureError::Panicked) => {
                        return Some(Err(FutureError::Panicked))
                    }
                    Ok(e) => return Some(Ok(e)),
                }
            }
            _OrElse::Second(ref mut b) => return b.poll(),
        };
        let ret = b.poll();
        self.future = _OrElse::Second(b);
        return ret
    }

    fn await(self) -> FutureResult<B::Item, B::Error> {
        match self.future {
            _OrElse::First(a, f) => {
                match or_else(a.await(), f) {
                    Err(FutureError::Other(e)) => e.into_future().await(),
                    Err(FutureError::Panicked) => Err(FutureError::Panicked),
                    Ok(e) => Ok(e),
                }
            }
            _OrElse::Second(b) => b.await(),
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<B::Item, B::Error>) + Send + 'static
    {
        match self.future {
            _OrElse::First(ref mut a, ref mut f) => {
                let f = f.take();
                a.schedule(|r| {
                    match or_else(r, f) {
                        Err(FutureError::Other(e)) => e.into_future().schedule(g),
                        Err(FutureError::Panicked) => g(Err(FutureError::Panicked)),
                        Ok(e) => g(Ok(e)),
                    }
                })
            }
            _OrElse::Second(ref mut b) => b.schedule(g)
        }
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

pub struct Empty<T, E> {
    _marker: marker::PhantomData<(T, E)>,
}

pub fn empty<T: Send + 'static, E: Send + 'static>() -> Empty<T, E> {
    Empty { _marker: marker::PhantomData }
}

impl<T, E> Future for Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<FutureResult<T, E>> {
        None
    }

    fn await(self) -> FutureResult<T, E> {
        panic!("cannot ever successfully await() on Empty")
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<T, E>) + Send + 'static
    {
        drop(g);
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        drop(cb);
    }
}

impl<T, E> Clone for Empty<T, E> {
    fn clone(&self) -> Empty<T, E> {
        Empty { _marker: marker::PhantomData }
    }
}

pub struct Select<A, B> {
    a: A,
    b: B,
}

impl<A, B> Future for Select<A, B>
    where A: Future,
          B: Future<Item=A::Item, Error=A::Error>,
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self) -> Option<FutureResult<A::Item, A::Error>> {
        self.a.poll().or_else(|| self.b.poll())
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<A::Item, A::Error>) + Send + 'static
    {
        let cell1 = Arc::new(cell::AtomicCell::new(Some(g)));
        let cell2 = cell1.clone();

        self.a.schedule(move |result| {
            let cell1 = cell1.borrow();
            if let Some(g) = cell1.and_then(|mut b| b.take()) {
                g(result);
            }
        });
        self.b.schedule(move |result| {
            let cell2 = cell2.borrow();
            if let Some(g) = cell2.and_then(|mut b| b.take()) {
                g(result);
            }
        });
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<A::Item, A::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

pub struct Join<A, B> where A: Future, B: Future<Error=A::Error> {
    a: A,
    b: B,
    a_res: Option<FutureResult<A::Item, A::Error>>,
    b_res: Option<FutureResult<B::Item, A::Error>>,
}

impl<A, B> Future for Join<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    type Item = (A::Item, B::Item);
    type Error = A::Error;

    fn poll(&mut self) -> Option<FutureResult<Self::Item, Self::Error>> {
        let a = self.a_res.take().or_else(|| self.a.poll());
        let b = self.b_res.take().or_else(|| self.b.poll());
        match (a, b) {
            // TODO: cancel the other?
            (Some(Err(e)), _) |
            (_, Some(Err(e))) => Some(Err(e)),

            (None, _) |
            (_, None) => None,
            (Some(Ok(a)), Some(Ok(b))) => Some(Ok((a, b))),
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<Self::Item, Self::Error>) + Send + 'static
    {
        match (self.a_res.take(), self.b_res.take()) {
            (Some(a), Some(b)) => {
                return g(a.and_then(|a| b.map(|b| (a, b))))
            }
            (Some(Ok(a)), None) => {
                return self.b.schedule(|res| g(res.map(|b| (a, b))))
            }
            (None, Some(Ok(b))) => {
                return self.a.schedule(|res| g(res.map(|a| (a, b))))
            }
            // TODO: cancel other?
            (Some(Err(e)), None) |
            (None, Some(Err(e))) => return g(Err(e)),

            (None, None) => {}
        }

        struct State<G, A, B, E> {
            a: cell::AtomicCell<Option<FutureResult<A, E>>>,
            b: cell::AtomicCell<Option<FutureResult<B, E>>>,
            g: cell::AtomicCounter<G>,
        }

        let data1 = Arc::new(State::<G, A::Item, B::Item, A::Error> {
            a: cell::AtomicCell::new(None),
            b: cell::AtomicCell::new(None),
            g: cell::AtomicCounter::new(g, 2),
        });
        let data2 = data1.clone();

        impl<G, A, B, E> State<G, A, B, E>
            where A: Send + 'static,
                  B: Send + 'static,
                  E: Send + 'static,
                  G: FnOnce(FutureResult<(A, B), E>),
        {
            fn finish(&self) {
                let g = match self.g.try_take() {
                    Some(g) => g,
                    None => return,
                };
                let a = self.a.borrow().unwrap().take().unwrap();
                let b = self.b.borrow().unwrap().take().unwrap();
                g(a.and_then(|a| b.map(|b| (a, b))))
            }
        }

        self.a.schedule(move |result| {
            // TODO: if we hit an error we should finish immediately
            *data1.a.borrow().unwrap() = Some(result);
            data1.finish();
        });
        self.b.schedule(move |result| {
            *data2.b.borrow().unwrap() = Some(result);
            data2.finish();
        });
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}
//
// // pub struct Cancellable<A> {
// //     future: A,
// //     canceled: bool,
// // }
// //
// // #[derive(Clone, Copy, PartialEq, Debug)]
// // pub enum CancelError<E> {
// //     Cancelled,
// //     Other(E),
// // }
// //
// // impl<A> Cancellable<A> {
// //     pub fn cancel(&mut self) {
// //         self.canceled = true;
// //     }
// // }
// //
// // impl<A> Future for Cancellable<A>
// //     where A: Future,
// // {
// //     type Item = A::Item;
// //     type Error = CancelError<A::Error>;
// //
// //     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
// //         if self.canceled {
// //             Ok(Err(CancelError::Cancelled))
// //         } else {
// //             match self.future.poll() {
// //                 Ok(res) => Ok(res.map_err(CancelError::Other)),
// //                 Err(e) => Err(e.cancellable())
// //             }
// //         }
// //     }
// // }

pub struct Collect<I> where I: Iterator, I::Item: Future {
    remaining: Option<I>,
    cur: Option<I::Item>,
    result: Vec<<I::Item as Future>::Item>,
}

pub fn collect<I>(i: I) -> Collect<I::IntoIter>
    where I: IntoIterator,
          I::Item: Future,
          I::IntoIter: Send + 'static,
{
    let mut i = i.into_iter();
    Collect { cur: i.next(), remaining: Some(i), result: Vec::new() }
}

impl<I> Future for Collect<I>
    where I: Iterator + Send + 'static,
          I::Item: Future,
{
    type Item = Vec<<I::Item as Future>::Item>;
    type Error = <I::Item as Future>::Error;

    fn poll(&mut self) -> Option<FutureResult<Self::Item, Self::Error>> {
        let remaining = match self.remaining.as_mut() {
            Some(i) => i,
            None => return Some(Err(FutureError::Panicked)),
        };
        loop {
            match self.cur.as_mut().map(Future::poll) {
                Some(Some(Ok(i))) => {
                    self.result.push(i);
                    self.cur = remaining.next();
                }
                Some(Some(Err(e))) => return Some(Err(e)),
                Some(None) => return None,
                None => return Some(Ok(mem::replace(&mut self.result, Vec::new()))),
            }
        }
    }

    fn await(self) -> FutureResult<Self::Item, Self::Error> {
        let Collect { remaining, cur, mut result } = self;
        let remaining = match remaining {
            Some(i) => i,
            None => return Err(FutureError::Panicked),
        };
        for item in cur.into_iter().chain(remaining) {
            result.push(try!(item.await()));
        }
        Ok(result)
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<Self::Item, Self::Error>) + Send + 'static
    {
        let mut result = mem::replace(&mut self.result, Vec::new());
        let mut next = match self.cur.take() {
            Some(next) => next,
            None => return g(Ok(result)),
        };
        let mut remaining = match self.remaining.take() {
            Some(i) => i,
            None => return g(Err(FutureError::Panicked)),
        };
        next.schedule(move |res| {
            let item = match res {
                Ok(i) => i,
                Err(e) => return g(Err(e)),
            };
            result.push(item);
            Collect {
                cur: remaining.next(),
                remaining: Some(remaining),
                result: result,
            }.schedule(g);
        })
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

pub struct Finished<T, E> {
    t: Option<T>,
    _e: marker::PhantomData<E>,
}

pub fn finished<T, E>(t: T) -> Finished<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Finished { t: Some(t), _e: marker::PhantomData }
}

impl<T, E> Future for Finished<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<FutureResult<T, E>> {
        match self.t.take() {
            Some(t) => Some(Ok(t)),
            None => Some(Err(FutureError::Panicked)),
        }
    }

    fn await(mut self) -> FutureResult<T, E> {
        self.poll().unwrap()
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<T, E>) + Send + 'static
    {
        g(self.poll().unwrap())
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        self.schedule(|r| cb.call(r))
    }
}

pub struct Failed<T, E> {
    _t: marker::PhantomData<T>,
    e: Option<E>,
}

pub fn failed<T, E>(e: E) -> Failed<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    Failed { _t: marker::PhantomData, e: Some(e) }
}

impl<T, E> Future for Failed<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<FutureResult<T, E>> {
        match self.e.take() {
            Some(e) => Some(Err(FutureError::Other(e))),
            None => Some(Err(FutureError::Panicked)),
        }
    }

    fn await(mut self) -> FutureResult<T, E> {
        self.poll().unwrap()
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<T, E>) + Send + 'static
    {
        g(self.poll().unwrap())
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<T, E>>) {
        self.schedule(|r| cb.call(r))
    }
}

pub struct Lazy<F, R> {
    inner: _Lazy<F, R>
}

enum _Lazy<F, R> {
    First(Option<F>),
    Second(R),
}

pub fn lazy<F, R>(f: F) -> Lazy<F, R>
    where F: FnOnce() -> R + Send + 'static,
          R: Future
{
    Lazy { inner: _Lazy::First(Some(f)) }
}

impl<F, R> Future for Lazy<F, R>
    where F: FnOnce() -> R + Send + 'static,
          R: Future
{
    type Item = R::Item;
    type Error = R::Error;

    fn poll(&mut self) -> Option<FutureResult<R::Item, R::Error>> {
        let mut future = match self.inner {
            _Lazy::First(ref mut f) => {
                match f.take() {
                    Some(f) => f(),
                    None => return Some(Err(FutureError::Panicked)),
                }
            }
            _Lazy::Second(ref mut f) => return f.poll(),
        };
        let ret = future.poll();
        self.inner = _Lazy::Second(future);
        return ret
    }

    fn await(self) -> FutureResult<R::Item, R::Error> {
        match self.inner {
            _Lazy::First(Some(f)) => f().await(),
            _Lazy::First(None) => Err(FutureError::Panicked),
            _Lazy::Second(f) => f.await(),
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<R::Item, R::Error>) + Send + 'static
    {
        let mut future = match self.inner {
            _Lazy::First(ref mut f) => {
                match f.take() {
                    Some(f) => f(),
                    None => return g(Err(FutureError::Panicked)),
                }
            }
            _Lazy::Second(ref mut f) => return f.schedule(g),
        };
        future.schedule(g);
        self.inner = _Lazy::Second(future);
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<R::Item, R::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}

pub struct Flatten<A> where A: Future, A::Item: IntoFuture {
    state: _Flatten<A, <A::Item as IntoFuture>::Future>,
}

enum _Flatten<A, B> {
    First(A),
    Second(B),
}

impl<A> Future for Flatten<A>
    where A: Future,
          A::Item: IntoFuture,
          <<A as Future>::Item as IntoFuture>::Error: From<<A as Future>::Error>
{
    type Item = <A::Item as IntoFuture>::Item;
    type Error = <A::Item as IntoFuture>::Error;

    fn poll(&mut self) -> Option<FutureResult<Self::Item, Self::Error>> {
        let mut b = match self.state {
            _Flatten::First(ref mut a) => {
                match a.poll() {
                    Some(Ok(a)) => a.into_future(),
                    Some(Err(e)) => return Some(Err(e.map(From::from))),
                    None => return None,
                }
            }
            _Flatten::Second(ref mut b) => return b.poll(),
        };
        let ret = b.poll();
        self.state = _Flatten::Second(b);
        return ret
    }

    fn await(self) -> FutureResult<Self::Item, Self::Error> {
        match self.state {
            _Flatten::First(a) => {
                match a.await() {
                    Ok(b) => b.into_future().await(),
                    Err(e) => Err(e.map(From::from)),
                }
            }
            _Flatten::Second(b) => b.await(),
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(FutureResult<Self::Item, Self::Error>) + Send + 'static
    {
        match self.state {
            _Flatten::First(ref mut a) => {
                a.schedule(|result| {
                    match result {
                        Ok(item) => item.into_future().schedule(g),
                        Err(e) => g(Err(e.map(From::from))),
                    }
                })
            }
            _Flatten::Second(ref mut b) => b.schedule(g),
        }
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}
