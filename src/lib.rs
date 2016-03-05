extern crate crossbeam;

// use std::sync::mpsc::{Receiver, RecvError, TryRecvError};
// use std::marker;
use std::sync::Arc;

use cell::AtomicCell;

pub mod cell;
pub mod mio;
// pub mod ivar;
pub mod slot;
pub mod promise;

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

pub trait Future: Send + 'static {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self>
        where Self: Sized;

    fn schedule<F>(self, f: F)
        where F: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static,
              Self: Sized;

    fn await(self) -> Result<Self::Item, Self::Error>
        where Self: Sized
    {
        let (a, b) = promise::pair();
        let mut slot = AtomicCell::new(None);
        let ptr = &mut slot as *mut _ as usize;
        self.schedule(move |result| {
            unsafe {
                let ptr: *mut AtomicCell<Option<Result<_, _>>> = ptr as *mut _;
                *(*ptr).borrow().unwrap() = Some(result);
            }
            b.finish(());
        });
        unit_await(a);
        let mut slot = slot.borrow().unwrap();
        return slot.take().unwrap();

        fn unit_await(p: promise::Promise<()>) {
            let res = mio::INNER.with(|l| l.await(p));
            assert!(res.is_ok());
        }
    }

    fn boxed(self) -> Box<Future<Item=Self::Item, Error=Self::Error>>
        where Self: Sized
    {
        Box::new(self)
    }

    fn map<F, U>(self, f: F) -> Map<Self, F>
        where F: FnOnce(Self::Item) -> U + Send + 'static,
              Self: Sized,
    {
        Map {
            future: self,
            f: f,
        }
    }

    // fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
    //     where F: FnOnce(Self::Error) -> E,
    //           Self: Sized,
    // {
    //     MapErr {
    //         future: self,
    //         f: f,
    //     }
    // }
    //
    // fn then<F, B>(self, f: F) -> Then<Self, B, F>
    //     where F: FnOnce(Result<Self::Item, Self::Error>) -> B,
    //           B: IntoFuture,
    //           Self: Sized,
    // {
    //     Then {
    //         future: _Then::First(self, f),
    //     }
    // }

    fn and_then<F, B>(self, f: F) -> AndThen<Self, B, F>
        where F: FnOnce(Self::Item) -> B,
              B: IntoFuture<Error = Self::Error>,
              Self: Sized,
    {
        AndThen {
            future: _AndThen::First(self, f),
        }
    }

    // fn or_else<F, B>(self, f: F) -> OrElse<Self, B, F>
    //     where F: FnOnce(Self::Error) -> B,
    //           B: IntoFuture<Item = Self::Item>,
    //           Self: Sized,
    // {
    //     OrElse {
    //         future: _OrElse::First(self, f),
    //     }
    // }

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
            state: _Join::Both(self, other.into_future()),
        }
    }

    // fn cancellable(self) -> Cancellable<Self> where Self: Sized {
    //     Cancellable {
    //         future: self,
    //         canceled: false,
    //     }
    // }
}

// #[derive(Copy, Clone)]
// pub struct FutureResult<T, E> {
//     inner: Result<T, E>,
// }
//
// impl<T, E> IntoFuture for Result<T, E> {
//     type Future = FutureResult<T, E>;
//     type Item = T;
//     type Error = E;
//
//     fn into_future(self) -> FutureResult<T, E> {
//         FutureResult { inner: self }
//     }
// }
//
// impl<T, E> Future for FutureResult<T, E> {
//     type Item = T;
//     type Error = E;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         Ok(self.inner)
//     }
// }

pub struct Map<A, F> {
    future: A,
    f: F,
}

impl<U, A, F> Future for Map<A, F>
    where A: Future,
          F: FnOnce(A::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = U;
    type Error = A::Error;

    fn poll(self) -> Result<Result<U, A::Error>, Self> {
        match self.future.poll() {
            Ok(result) => Ok(result.map(self.f)),
            Err(f) => Err(Map { future: f, f: self.f })
        }
    }

    // fn await(self) -> Result<U, A::Error> {
    //     self.future.await().map(self.f)
    // }

    fn schedule<G>(self, g: G)
        where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
    {
        let Map { future, f } = self;
        future.schedule(move |result| g(result.map(f)))
    }
}

// pub struct MapErr<A, F> {
//     future: A,
//     f: F,
// }
//
// impl<A, E, F> Future for MapErr<A, F>
//     where A: Future,
//           F: FnOnce(A::Error) -> E,
// {
//     type Item = A::Item;
//     type Error = E;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         match self.future.poll() {
//             Ok(result) => Ok(result.map_err(self.f)),
//             Err(f) => Err(MapErr { future: f, f: self.f })
//         }
//     }
// }
//
// pub struct Then<A, B, F> where B: IntoFuture {
//     future: _Then<A, B::Future, F>,
// }
//
// enum _Then<A, B, F> {
//     First(A, F),
//     Second(B),
// }
//
// impl<A, B, F> Future for Then<A, B, F>
//     where A: Future,
//           B: IntoFuture,
//           F: FnOnce(Result<A::Item, A::Error>) -> B,
// {
//     type Item = B::Item;
//     type Error = B::Error;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         let second = match self.future {
//             _Then::First(a, f) => {
//                 match a.poll() {
//                     Ok(next) => f(next).into_future(),
//                     Err(a) => {
//                         return Err(Then {
//                             future: _Then::First(a, f),
//                         })
//                     }
//                 }
//             }
//             _Then::Second(b) => b,
//         };
//         second.poll().map_err(|b| {
//             Then { future: _Then::Second(b) }
//         })
//     }
// }

pub struct AndThen<A, B, F> where B: IntoFuture {
    future: _AndThen<A, B::Future, F>,
}

enum _AndThen<A, B, F> {
    First(A, F),
    Second(B),
}

impl<A, B, F> Future for AndThen<A, B, F>
    where A: Future,
          B: IntoFuture<Error = A::Error>,
          F: FnOnce(A::Item) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        let second = match self.future {
            _AndThen::First(a, f) => {
                match a.poll() {
                    Ok(Ok(next)) => f(next).into_future(),
                    Ok(Err(e)) => return Ok(Err(e)),
                    Err(a) => {
                        return Err(AndThen {
                            future: _AndThen::First(a, f),
                        })
                    }
                }
            }
            _AndThen::Second(b) => b,
        };
        second.poll().map_err(|b| {
            AndThen { future: _AndThen::Second(b) }
        })
    }

    // fn await(self) -> Result<Self::Item, Self::Error> {
    //     let second = match self.future {
    //         _AndThen::First(a, f) => f(try!(a.await())).into_future(),
    //         _AndThen::Second(b) => b,
    //     };
    //     second.await()
    // }

    fn schedule<G>(self, g: G)
        where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
    {
        match self.future {
            _AndThen::First(a, f) => {
                a.schedule(move |res| {
                    match res.map(f).map(|f| f.into_future()) {
                        Ok(future) => future.schedule(g),
                        Err(e) => g(Err(e)),
                    }
                })
            }
            _AndThen::Second(b) => b.schedule(g),
        }
    }
}

// pub struct OrElse<A, B, F> where B: IntoFuture {
//     future: _OrElse<A, B::Future, F>,
// }
//
// enum _OrElse<A, B, F> {
//     First(A, F),
//     Second(B),
// }
//
// impl<A, B, F> Future for OrElse<A, B, F>
//     where A: Future,
//           B: IntoFuture<Item = A::Item>,
//           F: FnOnce(A::Error) -> B,
// {
//     type Item = B::Item;
//     type Error = B::Error;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         let second = match self.future {
//             _OrElse::First(a, f) => {
//                 match a.poll() {
//                     Ok(Ok(next)) => return Ok(Ok(next)),
//                     Ok(Err(e)) => f(e).into_future(),
//                     Err(a) => {
//                         return Err(OrElse {
//                             future: _OrElse::First(a, f),
//                         })
//                     }
//                 }
//             }
//             _OrElse::Second(b) => b,
//         };
//         second.poll().map_err(|b| {
//             OrElse { future: _OrElse::Second(b) }
//         })
//     }
// }
//
// pub struct Empty<T, E> {
//     _marker: marker::PhantomData<(T, E)>,
// }
//
// pub fn empty<T, E>() -> Empty<T, E> {
//     Empty { _marker: marker::PhantomData }
// }
//
// impl<T, E> Future for Empty<T, E> {
//     type Item = T;
//     type Error = E;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         Err(self)
//     }
// }
//
// impl<T, E> Clone for Empty<T, E> {
//     fn clone(&self) -> Empty<T, E> {
//         empty()
//     }
// }
//
// impl<T, E> Copy for Empty<T, E> {}

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

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        let Select { a, b } = self;
        a.poll().or_else(|a| {
            b.poll().map_err(|b| {
                Select { a: a, b: b }
            })
        })
    }

    fn schedule<G>(self, g: G)
        where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
    {
        let Select { a, b } = self;
        let cell1 = Arc::new(cell::AtomicCell::new(Some(g)));
        let cell2 = cell1.clone();

        a.schedule(move |result| {
            if let Ok(Some(g)) = cell1.replace(None) {
                g(result);
            }
        });
        b.schedule(move |result| {
            if let Ok(Some(g)) = cell2.replace(None) {
                g(result);
            }
        });
    }
}

pub struct Join<A, B> where A: Future, B: Future<Error=A::Error> {
    state: _Join<A, B>,
}

enum _Join<A, B> where A: Future, B: Future<Error=A::Error> {
    Both(A, B),
    First(A, Result<B::Item, A::Error>),
    Second(Result<A::Item, A::Error>, B),
}

impl<A, B> Future for Join<A, B>
    where A: Future,
          B: Future<Error=A::Error>,
{
    type Item = (A::Item, B::Item);
    type Error = A::Error;

    fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
        let (a, b) = match self.state {
            _Join::Both(a, b) => (a.poll(), b.poll()),
            _Join::First(a, b) => (a.poll(), Ok(b)),
            _Join::Second(a, b) => (Ok(a), b.poll()),
        };
        match (a, b) {
            (Ok(Err(e)), _) |
            (_, Ok(Err(e))) => Ok(Err(e)),
            (Ok(Ok(a)), Ok(Ok(b))) => Ok(Ok((a, b))),
            (Err(a), Ok(b)) => {
                Err(Join { state: _Join::First(a, b) })
            }
            (Ok(a), Err(b)) => {
                Err(Join { state: _Join::Second(a, b) })
            }
            (Err(a), Err(b)) => {
                Err(Join { state: _Join::Both(a, b) })
            }
        }
    }

    // fn await(self) -> Result<Self::Item, Self::Error> {
    //     match self.state {
    //         _Join::Both(a, b) => {
    //             a.await().and_then(|a| {
    //                 b.await().map(|b| (a, b))
    //             })
    //         }
    //         _Join::First(a, b) => {
    //             b.and_then(|b| {
    //                 a.await().map(|a| (a, b))
    //             })
    //         }
    //         _Join::Second(a, b) => {
    //             a.and_then(|a| {
    //                 b.await().map(|b| (a, b))
    //             })
    //         }
    //     }
    // }

    fn schedule<G>(self, g: G)
        where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
    {
        match self.state {
            _Join::Both(a, b) => {
                struct State<G, A, B> where A: Future, B: Future {
                    a: cell::AtomicCell<Option<Result<A::Item, A::Error>>>,
                    b: cell::AtomicCell<Option<Result<B::Item, B::Error>>>,
                    g: cell::AtomicCounter<G>,
                }

                let data1 = Arc::new(State::<G, A, B> {
                    a: cell::AtomicCell::new(None),
                    b: cell::AtomicCell::new(None),
                    g: cell::AtomicCounter::new(g, 2),
                });
                let data2 = data1.clone();

                impl<G, A, B> State<G, A, B>
                    where A: Future,
                          B: Future<Error=A::Error>,
                          G: FnOnce(Result<(A::Item, B::Item), A::Error>),
                {
                    fn finish(&self) {
                        let g = match self.g.try_take() {
                            Some(g) => g,
                            None => return,
                        };
                        let a = self.a.replace(None).ok().unwrap().unwrap();
                        let b = self.b.replace(None).ok().unwrap().unwrap();
                        g(a.and_then(|a| b.map(|b| (a, b))))
                    }
                }

                a.schedule(move |result| {
                    // TODO: if we hit an error we should finish immediately
                    assert!(data1.a.replace(Some(result)).is_ok());
                    data1.finish();
                });
                b.schedule(move |result| {
                    assert!(data2.b.replace(Some(result)).is_ok());
                    data2.finish();
                });
            }
            _Join::First(a, b) => {
                match b {
                    Ok(b) => a.schedule(|res| g(res.map(|a| (a, b)))),
                    Err(e) => g(Err(e)),
                }
            }
            _Join::Second(a, b) => {
                match a {
                    Ok(a) => b.schedule(|res| g(res.map(|b| (a, b)))),
                    Err(e) => g(Err(e)),
                }
            }
        }
    }
}

// pub struct Cancellable<A> {
//     future: A,
//     canceled: bool,
// }
//
// #[derive(Clone, Copy, PartialEq, Debug)]
// pub enum CancelError<E> {
//     Cancelled,
//     Other(E),
// }
//
// impl<A> Cancellable<A> {
//     pub fn cancel(&mut self) {
//         self.canceled = true;
//     }
// }
//
// impl<A> Future for Cancellable<A>
//     where A: Future,
// {
//     type Item = A::Item;
//     type Error = CancelError<A::Error>;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         if self.canceled {
//             Ok(Err(CancelError::Cancelled))
//         } else {
//             match self.future.poll() {
//                 Ok(res) => Ok(res.map_err(CancelError::Other)),
//                 Err(e) => Err(e.cancellable())
//             }
//         }
//     }
// }
//
// pub struct Collect<I> where I: IntoIterator, I::Item: Future {
//     remaining: I::IntoIter,
//     cur: Option<I::Item>,
//     result: Vec<<I::Item as Future>::Item>,
// }
//
// pub fn collect<I>(i: I) -> Collect<I>
//     where I: IntoIterator,
//           I::Item: Future,
// {
//     let mut i = i.into_iter();
//     Collect { cur: i.next(), remaining: i, result: Vec::new() }
// }
//
// impl<I> Future for Collect<I>
//     where I: IntoIterator,
//           I::Item: Future,
// {
//     type Item = Vec<<I::Item as Future>::Item>;
//     type Error = <I::Item as Future>::Error;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         let Collect { mut remaining, mut cur, mut result } = self;
//         loop {
//             match cur.map(|c| c.poll()) {
//                 Some(Ok(Ok(i))) => {
//                     result.push(i);
//                     cur = remaining.next();
//                 }
//                 Some(Ok(Err(e))) => return Ok(Err(e)),
//                 Some(Err(e)) => {
//                     return Err(Collect {
//                         remaining: remaining,
//                         cur: Some(e),
//                         result: result,
//                     })
//                 }
//                 None => return Ok(Ok(result)),
//             }
//         }
//     }
// }
//
// pub struct Finished<T, E> {
//     t: T,
//     _e: marker::PhantomData<E>,
// }
//
// pub fn finished<T, E>(t: T) -> Finished<T, E> {
//     Finished { t: t, _e: marker::PhantomData }
// }
//
// impl<T, E> Future for Finished<T, E> {
//     type Item = T;
//     type Error = E;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         Ok(Ok(self.t))
//     }
// }
//
// pub struct Failed<T, E> {
//     _t: marker::PhantomData<T>,
//     e: E,
// }
//
// pub fn failed<T, E>(e: E) -> Failed<T, E> {
//     Failed { _t: marker::PhantomData, e: e }
// }
//
// impl<T, E> Future for Failed<T, E> {
//     type Item = T;
//     type Error = E;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         Ok(Err(self.e))
//     }
// }
//
// pub struct Lazy<F, R> {
//     inner: _Lazy<F, R>
// }
//
// enum _Lazy<F, R> {
//     First(F),
//     Second(R),
// }
//
// pub fn lazy<F, R>(f: F) -> Lazy<F, R>
//     where F: FnOnce() -> R,
//           R: Future
// {
//     Lazy { inner: _Lazy::First(f) }
// }
//
// impl<F, R> Future for Lazy<F, R>
//     where F: FnOnce() -> R,
//           R: Future
// {
//     type Item = R::Item;
//     type Error = R::Error;
//
//     fn poll(self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         let future = match self.inner {
//             _Lazy::First(f) => f(),
//             _Lazy::Second(f) => f,
//         };
//         future.poll().map_err(|f| {
//             Lazy { inner: _Lazy::Second(f) }
//         })
//     }
// }
