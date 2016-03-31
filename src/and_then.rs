use {Future, IntoFuture, PollError, Callback, PollResult, FutureResult};
use FutureError;
use Then;
use util;

pub struct AndThen<A, B, F> {
    inner: Then<A, B, MyThing<F>>,
}

struct MyThing<F>(F);

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F> {
    AndThen {
        inner: future.then(MyThing(f)),
    }
}

impl<A, B, F> Future for AndThen<A, B::Future, F>
    where A: Future,
          B: IntoFuture<Error = A::Error>,
          F: FnOnce(A::Item) -> B + Send + 'static,
{
    type Item = B::Item;
    type Error = B::Error;

    fn poll(&mut self) -> Option<PollResult<B::Item, B::Error>> {
        self.inner.poll()
    }

    fn await(&mut self) -> FutureResult<B::Item, B::Error> {
        self.inner.await()
    }

    fn cancel(&mut self) {
        self.inner.cancel()
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<B::Item, B::Error>) + Send + 'static
    {
        self.inner.schedule(g)
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<B::Item, B::Error>>) {
        self.inner.schedule_boxed(cb)
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
