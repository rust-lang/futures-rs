
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
